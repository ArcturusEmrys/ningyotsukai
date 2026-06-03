//! Management for render targets

use glam::Vec2;
use inox2d::math::camera::Camera;
use ningyo_extensions::CurrentSurfaceTextureExt;
use wgpu::util::DeviceExt;

use crate::camera::CameraExt;
use crate::error::WgpuRendererError;
use crate::shader::UniformBlock;
use crate::shaders::basic::basic_frag::{Viewport, Viewports};
use crate::texture::{DepthStencilTexture, DeviceTexture, GBuffer};

/// A particular viewport's configuration.
struct ViewportConfiguration {
    /// The specific viewport camera for this viewport.
    view: Camera,

    /// The width and height of the viewport to render.
    size: (u32, u32),
}

impl ViewportConfiguration {
    fn as_shader_viewport(&self) -> Viewport {
        Viewport {
            projection: self
                .view
                .to_view_proj_matrix(Vec2::new(self.size.0 as f32, self.size.1 as f32))
                .to_cols_array_2d(),
            scissor_origin_tl: [0.0; 2],
            scissor_origin_br: [self.size.0 as f32, self.size.1 as f32],
        }
    }
}

/// A render target for color output.
enum ColorOutput {
    Surface,
    Texture(DeviceTexture),
}

/// The list of desired viewport configurations for this set of targets.
///
/// Note that for surface rendering (where we render to an external window
/// surface), only one rendering output is supported.
enum OutputConfiguration<'surf> {
    // We intend to render to a surface of the given configuration.
    Surface((wgpu::Surface<'surf>, wgpu::SurfaceConfiguration, Camera)),

    // We intend to render to a 2D array texture, whose layers will be cut up
    // as described in the ViewportConfiguration.
    Texture(Vec<ViewportConfiguration>),

    // We intend to render to a user-provided 2D texture.
    UserTarget((DeviceTexture, Camera)),
}

impl<'surf> OutputConfiguration<'surf> {
    /// Calculate the maximum width of all viewports.
    fn max_width(&self) -> u32 {
        match self {
            OutputConfiguration::Surface((_, config, _)) => config.width,
            OutputConfiguration::Texture(viewports) => {
                let mut max = 0;

                for viewport in viewports {
                    max = std::cmp::max(max, viewport.size.0);
                }

                max
            }
            OutputConfiguration::UserTarget((texture, _)) => texture.texture().width(),
        }
    }

    /// Calculate the maximum height of all viewports.
    fn max_height(&self) -> u32 {
        match self {
            OutputConfiguration::Surface((_, config, _)) => config.height,
            OutputConfiguration::Texture(viewports) => {
                let mut max = 0;

                for viewport in viewports {
                    max = std::cmp::max(max, viewport.size.1);
                }

                max
            }
            OutputConfiguration::UserTarget((texture, _)) => texture.texture().height(),
        }
    }

    /// Calculate the number of required layers for rendering.
    fn layers(&self) -> u32 {
        match self {
            OutputConfiguration::Surface(_) | OutputConfiguration::UserTarget(_) => 1,
            OutputConfiguration::Texture(viewports) => viewports.len() as u32,
        }
    }

    fn as_viewport_config(&self) -> Vec<Viewport> {
        match self {
            OutputConfiguration::Surface((_, config, camera)) => {
                vec![Viewport {
                    projection: camera
                        .to_view_proj_matrix(Vec2::new(config.width as f32, config.height as f32))
                        .to_cols_array_2d(),
                    scissor_origin_tl: [0.0; 2],
                    scissor_origin_br: [config.width as f32, config.height as f32],
                }]
            }
            OutputConfiguration::UserTarget((target, camera)) => {
                vec![Viewport {
                    projection: camera
                        .to_view_proj_matrix(Vec2::new(
                            target.texture().width() as f32,
                            target.texture().height() as f32,
                        ))
                        .to_cols_array_2d(),
                    scissor_origin_tl: [0.0; 2],
                    scissor_origin_br: [
                        target.texture().width() as f32,
                        target.texture().height() as f32,
                    ],
                }]
            }
            OutputConfiguration::Texture(viewports) => {
                let mut out = vec![];

                for viewport in viewports {
                    out.push(viewport.as_shader_viewport())
                }

                out
            }
        }
    }
}

/// An allocated set of output textures for a given renderer context.
struct RenderOutput {
    /// The output render target for color.
    ///
    /// Note that if we are rendering to a surface, we can only render one
    /// viewport.
    color_target: ColorOutput,

    /// The stencil target for output rendering.
    stencil_target: DepthStencilTexture,

    /// All targets for composite rendering.
    composite_target: GBuffer,
}

impl RenderOutput {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, config: &OutputConfiguration) -> Self {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Ningyo WGPU Render Output Texture Creation"),
        });

        let width = config.max_width();
        let height = config.max_height();
        let layers = config.layers();

        let me = Self {
            color_target: match config {
                OutputConfiguration::Surface(_) => ColorOutput::Surface,
                OutputConfiguration::Texture(_) => {
                    ColorOutput::Texture(DeviceTexture::empty_render_target(
                        device,
                        &mut encoder,
                        width,
                        height,
                        layers,
                        wgpu::TextureFormat::Rgba8Unorm,
                    ))
                }
                OutputConfiguration::UserTarget((target, _)) => {
                    ColorOutput::Texture(target.clone())
                }
            },
            stencil_target: DepthStencilTexture::empty_render_target(
                device,
                &mut encoder,
                width,
                height,
                layers,
                wgpu::TextureFormat::Depth24PlusStencil8,
            ),
            composite_target: GBuffer::new(
                device,
                &mut encoder,
                width,
                height,
                layers,
                //TODO: Wait, why? Nothing we work with is HDR.
                wgpu::TextureFormat::Rgba16Float,
                //TODO: You know wgpu has a stencil only format, right?
                wgpu::TextureFormat::Depth24PlusStencil8,
            ),
        };

        queue.submit(std::iter::once(encoder.finish()));

        me
    }

    /// Determine if the given render configuration is compatible with the
    /// current set of output textures.
    ///
    /// Note that this permits render outputs that are *larger* than the
    /// current set of textures. The Viewports array handed to shaders includes
    /// parameters for psuedo-scissoring the output, which you will need to
    /// configure.
    fn is_compatible_with_configuration(&self, config: &OutputConfiguration) -> bool {
        let reqd_width = config.max_width();
        let reqd_height = config.max_height();
        let reqd_layers = config.layers();

        (match config {
            OutputConfiguration::Surface(_config) => {
                matches!(self.color_target, ColorOutput::Surface)
            }
            OutputConfiguration::Texture(_viewports) => match &self.color_target {
                ColorOutput::Surface => false,
                ColorOutput::Texture(texture) => {
                    self.stencil_target.texture().width() == texture.texture().width()
                        && self.stencil_target.texture().height() == texture.texture().height()
                        && self.stencil_target.texture().depth_or_array_layers()
                            == texture.texture().depth_or_array_layers()
                }
            },
            OutputConfiguration::UserTarget((target, _)) => match &self.color_target {
                ColorOutput::Surface => false,
                ColorOutput::Texture(tex) => {
                    tex.texture() == target.texture()
                        && target.texture().height() == self.stencil_target.texture().height()
                        && target.texture().width() == self.stencil_target.texture().width()
                        && target.texture().depth_or_array_layers()
                            == self.stencil_target.texture().depth_or_array_layers()
                }
            },
        }) && self.composite_target.albedo().texture().width()
            == self.stencil_target.texture().width()
            && self.composite_target.albedo().texture().height()
                == self.stencil_target.texture().height()
            && self
                .composite_target
                .albedo()
                .texture()
                .depth_or_array_layers()
                == self.stencil_target.texture().depth_or_array_layers()
            && self.stencil_target.texture().width() >= reqd_width
            && self.stencil_target.texture().height() >= reqd_height
            && self.stencil_target.texture().depth_or_array_layers() >= reqd_layers
    }
}

/// A render target configuration and its associated output textures.
///
/// Surface textures are not allocated until requested. This means render
/// target creation is actually three phases:
///
/// 1. Create the desired render target type by calling `new_texture_target` or
/// `new_with_surface`.
///
/// 2. Provide a valid viewport configuration by calling `resize`.
///
/// 3. Call .apply() to apply the given configuration.
///
/// RenderTarget can also manage a multi-view configuration for rendering to an
/// array target. This is only supported for texture targets, *not* surface
/// targets, which are permanently locked to 1 render target layer.
pub struct RenderTarget<'surf> {
    /// The desired output configuration.
    config: OutputConfiguration<'surf>,

    /// The currently allocated render targets.
    outputs: Option<RenderOutput>,

    /// The viewport configuration to be sent to shaders.
    viewports_buffer: Option<wgpu::Buffer>,
}

impl<'surf> RenderTarget<'surf> {
    pub fn new_with_surface(surface: wgpu::Surface<'surf>, adapter: &wgpu::Adapter) -> Self {
        // Find a suitable surface configuration.
        let surface_caps = surface.get_capabilities(&adapter);
        let mut surface_format = surface_caps.formats[0];
        let non_srgb_surface = surface_caps.formats[0].remove_srgb_suffix();

        // SRGB makes blending look funny.
        if surface_caps
            .formats
            .iter()
            .find(|fmt| **fmt == non_srgb_surface)
            .is_some()
        {
            surface_format = non_srgb_surface;
        }

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,

            //TODO: We don't know the size of our surface at init time.
            width: 640,
            height: 480,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        RenderTarget {
            outputs: None,
            config: OutputConfiguration::Surface((surface, config, Camera::default())),
            viewports_buffer: None,
        }
    }

    pub fn new_texture_target() -> Self {
        RenderTarget {
            config: OutputConfiguration::Texture(vec![]),
            outputs: None,
            viewports_buffer: None,
        }
    }

    /// Resize the render target.
    ///
    /// For surface targets, this sets the width and height of the surface
    /// configuration. For viewports, this sets the width and height of the
    /// first viewport.
    ///
    /// The viewport parameter should always be 0 for surfaces and user-provided
    /// textures. For renderer-allocated targets, you may specify multiple
    /// viewports. Specifying a viewport at the end of the current list will
    /// append a viewport of that size to the list.
    pub fn resize(&mut self, width: u32, height: u32, viewport: usize) {
        if width > 0 && height > 0 {
            match &mut self.config {
                OutputConfiguration::Surface((_, config, _)) => {
                    config.width = width;
                    config.height = height;
                }
                OutputConfiguration::Texture(viewports) => {
                    if let Some(viewport) = viewports.get_mut(viewport) {
                        viewport.size = (width, height);
                    } else if viewports.len() == viewport {
                        viewports.push(ViewportConfiguration {
                            view: Camera::default(),
                            size: (width, height),
                        });
                    }
                }
                // User target size is defined by the provided texture.
                OutputConfiguration::UserTarget(_) => {}
            }
        }
    }

    /// Retrieve a given viewport's camera structure.
    ///
    /// Surface and user-provided render targets have one fixed viewport in
    /// slot 0. For renderer-allocated targets, any viewport that has been
    /// previously defined by a call to `resize` may have its camera altered.
    pub fn viewport_camera(&self, viewport: usize) -> Option<&Camera> {
        match &self.config {
            OutputConfiguration::Surface(_) => None,
            OutputConfiguration::UserTarget((_, camera)) => Some(camera),
            OutputConfiguration::Texture(viewports) => viewports.get(viewport).map(|vp| &vp.view),
        }
    }

    /// Retrieve a given viewport's camera structure for mutation.
    ///
    /// Surface and user-provided render targets have one fixed viewport in
    /// slot 0. For renderer-allocated targets, any viewport that has been
    /// previously defined by a call to `resize` may have its camera altered.
    pub fn viewport_camera_mut(&mut self, viewport: usize) -> Option<&mut Camera> {
        match &mut self.config {
            OutputConfiguration::Surface(_) => None,
            OutputConfiguration::UserTarget((_, camera)) => Some(camera),
            OutputConfiguration::Texture(viewports) => {
                viewports.get_mut(viewport).map(|vp| &mut vp.view)
            }
        }
    }

    /// Provide a user-specified texture as a render target.
    ///
    /// The texture must have been created with the texture usages in
    /// `required_render_target_uses` and must originate from the same device
    /// that we are using to render with.
    pub fn set_render_target(&mut self, target: wgpu::Texture) -> Result<(), WgpuRendererError> {
        let camera = self.viewport_camera(0).cloned().unwrap_or_default();
        self.config =
            OutputConfiguration::UserTarget((DeviceTexture::user_render_target(target)?, camera));

        Ok(())
    }

    /// Apply prior configuration changes.
    pub fn apply(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.outputs.is_none()
            || !self
                .outputs
                .as_ref()
                .unwrap()
                .is_compatible_with_configuration(&self.config)
        {
            self.outputs = Some(RenderOutput::new(device, queue, &self.config));
        }

        //TODO: We should have a flag to check if the viewport configuration
        //was actually mutated or not.

        let viewports_config = self.config.as_viewport_config();
        let viewports_config = Viewports {
            active_viewports: viewports_config.len() as u32,
            viewports: viewports_config,
        };

        let mut data = vec![0; viewports_config.required_size()];
        viewports_config.write_buffer(&mut data[..]);

        if let Some(buffer) = self.viewports_buffer.as_ref() {
            if buffer.size() >= viewports_config.required_size() as u64 {
                queue.write_buffer(buffer, 0, &data[..]);
                return;
            }
        }

        self.viewports_buffer = Some(device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Viewport configuration buffer init"),
                contents: &data[..],
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            },
        ));
    }

    /// Retrieve the current surface, if surface rendering is enabled.
    pub fn surface(&self) -> Option<&wgpu::Surface<'surf>> {
        match &self.config {
            OutputConfiguration::Surface((surface, _, _)) => Some(surface),
            _ => None,
        }
    }

    /// Get the color target view for rendering.
    pub fn color_target(&self) -> Result<wgpu::Texture, WgpuRendererError> {
        match &self.config {
            OutputConfiguration::Surface((surface, _, _)) => Ok(surface
                .get_current_texture()
                .as_surface_texture()?
                .texture
                .texture
                .clone()),
            OutputConfiguration::UserTarget((target, _)) => Ok(target.texture().clone()),
            OutputConfiguration::Texture(_) => {
                match &self
                    .outputs
                    .as_ref()
                    .ok_or(WgpuRendererError::ViewportNotInitialized)?
                    .color_target
                {
                    ColorOutput::Surface => unreachable!(),
                    ColorOutput::Texture(target) => Ok(target.texture().clone()),
                }
            }
        }
    }

    /// Get the color target view for rendering.
    pub fn color_target_view(&self) -> Result<wgpu::TextureView, WgpuRendererError> {
        match &self.config {
            OutputConfiguration::Surface((surface, _, _)) => Ok(surface
                .get_current_texture()
                .as_surface_texture()?
                .texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default())),
            OutputConfiguration::UserTarget((target, _)) => Ok(target.view().clone()),
            OutputConfiguration::Texture(_) => {
                match &self
                    .outputs
                    .as_ref()
                    .ok_or(WgpuRendererError::ViewportNotInitialized)?
                    .color_target
                {
                    ColorOutput::Surface => unreachable!(),
                    ColorOutput::Texture(target) => Ok(target.view().clone()),
                }
            }
        }
    }

    /// Get the compositing buffers.
    pub fn composite(&self) -> Result<&GBuffer, WgpuRendererError> {
        self.outputs
            .as_ref()
            .map(|o| &o.composite_target)
            .ok_or(WgpuRendererError::ViewportNotInitialized)
    }

    /// Get the stencil buffer.
    pub fn stencil(&self) -> Result<&DepthStencilTexture, WgpuRendererError> {
        self.outputs
            .as_ref()
            .map(|o| &o.stencil_target)
            .ok_or(WgpuRendererError::ViewportNotInitialized)
    }

    /// Issue a clear command on the output color buffer.
    pub fn clear(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), WgpuRendererError> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("WGPURenderer::clear"),
        });

        encoder.clear_texture(
            &self.color_target()?,
            &wgpu::ImageSubresourceRange {
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            },
        );

        queue.submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    /// Get the buffer containing the viewport configuration data for shaders.
    pub fn viewports_config(&self) -> Result<&wgpu::Buffer, WgpuRendererError> {
        self.viewports_buffer
            .as_ref()
            .ok_or(WgpuRendererError::ViewportNotInitialized)
    }
}
