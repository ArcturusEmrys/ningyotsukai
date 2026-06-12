//! Management for render targets

use std::cmp::min;

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
    fn as_shader_viewport(&self, target_viewport_size: Vec2) -> Viewport {
        Viewport {
            projection: self
                .view
                .to_view_proj_matrix(target_viewport_size)
                .to_cols_array_2d(),
            scissor_origin_tl: [0.0; 2],
            scissor_origin_br: [self.size.0 as f32, self.size.1 as f32],
            padding: Default::default(),
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
    Surface(wgpu::Surface<'surf>, wgpu::SurfaceConfiguration, Camera),

    // We intend to render to a 2D array texture, whose layers will be cut up
    // as described in the ViewportConfiguration.
    Texture(Vec<ViewportConfiguration>, wgpu::TextureFormat),

    // We intend to render to a user-provided 2D texture.
    UserTarget(DeviceTexture, Camera),
}

impl<'surf> OutputConfiguration<'surf> {
    /// Calculate the maximum width of all viewports.
    fn max_width(&self) -> u32 {
        match self {
            OutputConfiguration::Surface(_, config, _) => config.width,
            OutputConfiguration::Texture(viewports, _) => {
                let mut max = 0;

                for viewport in viewports {
                    max = std::cmp::max(max, viewport.size.0);
                }

                max
            }
            OutputConfiguration::UserTarget(texture, _) => texture.texture().width(),
        }
    }

    /// Calculate the maximum height of all viewports.
    fn max_height(&self) -> u32 {
        match self {
            OutputConfiguration::Surface(_, config, _) => config.height,
            OutputConfiguration::Texture(viewports, _) => {
                let mut max = 0;

                for viewport in viewports {
                    max = std::cmp::max(max, viewport.size.1);
                }

                max
            }
            OutputConfiguration::UserTarget(texture, _) => texture.texture().height(),
        }
    }

    /// Calculate the number of required layers for rendering.
    fn layers(&self) -> u32 {
        match self {
            OutputConfiguration::Surface(_, _, _) | OutputConfiguration::UserTarget(_, _) => 1,
            OutputConfiguration::Texture(viewports, _) => viewports.len() as u32,
        }
    }

    fn as_viewport_config(&self, output: Option<&RenderOutput>) -> Vec<Viewport> {
        match self {
            OutputConfiguration::Surface(_, config, camera) => {
                vec![Viewport {
                    projection: camera
                        .to_view_proj_matrix(Vec2::new(config.width as f32, config.height as f32))
                        .to_cols_array_2d(),
                    scissor_origin_tl: [0.0; 2],
                    scissor_origin_br: [config.width as f32, config.height as f32],
                    padding: Default::default(),
                }]
            }
            OutputConfiguration::UserTarget(target, camera) => {
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
                    padding: Default::default(),
                }]
            }
            OutputConfiguration::Texture(viewports, _) => {
                let embedded_size = if let Some(output) = output {
                    let ColorOutput::Texture(tex) = &output.color_target else {
                        unreachable!()
                    };
                    Vec2::new(tex.texture().width() as f32, tex.texture().height() as f32)
                } else {
                    Vec2::new(self.max_width() as f32, self.max_height() as f32)
                };
                let mut out = vec![];

                for viewport in viewports {
                    out.push(viewport.as_shader_viewport(embedded_size))
                }

                out
            }
        }
    }

    /// Get the position and size of a given viewport.
    fn as_origin_and_extent(&self, viewport: usize) -> Option<(wgpu::Origin3d, wgpu::Extent3d)> {
        match self {
            OutputConfiguration::Surface(_, config, _) => {
                if viewport > 0 {
                    return None;
                }

                Some((
                    wgpu::Origin3d::ZERO,
                    wgpu::Extent3d {
                        width: config.width,
                        height: config.height,
                        depth_or_array_layers: 1,
                    },
                ))
            }
            OutputConfiguration::Texture(config, _) => {
                let config = config.get(viewport)?;
                Some((
                    wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: viewport as u32,
                    },
                    wgpu::Extent3d {
                        width: config.size.0,
                        height: config.size.1,
                        depth_or_array_layers: 1,
                    },
                ))
            }
            OutputConfiguration::UserTarget(texture, _) => {
                if viewport > 0 {
                    return None;
                }

                Some((
                    wgpu::Origin3d::ZERO,
                    wgpu::Extent3d {
                        width: texture.texture().width(),
                        height: texture.texture().height(),
                        depth_or_array_layers: 1,
                    },
                ))
            }
        }
    }
}

/// An allocated set of output textures for a given renderer context.
pub struct RenderOutput {
    /// The output render target for color.
    ///
    /// Note that if we are rendering to a surface, we can only render one
    /// viewport.
    color_target: ColorOutput,

    /// The stencil target for output rendering.
    stencil_target: DepthStencilTexture,

    /// All targets for composite rendering.
    composite_target: GBuffer,

    /// Individual layer views for the color texture.
    color_layer_views: Vec<wgpu::TextureView>,

    /// Individual layer views for the stencil target.
    stencil_layer_views: Vec<wgpu::TextureView>,

    /// Individual layer views for the composite buffer's albedo target.
    composite_albedo_layer_views: Vec<wgpu::TextureView>,

    /// Individual layer views for the composite buffer's emissive target.
    composite_emissive_layer_views: Vec<wgpu::TextureView>,

    /// Individual layer views for the composite buffer's bump target.
    composite_bump_layer_views: Vec<wgpu::TextureView>,

    /// Individual layer views for the composite buffer's stencil target.
    composite_stencil_layer_views: Vec<wgpu::TextureView>,
}

impl RenderOutput {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        config: &OutputConfiguration,
    ) -> Result<Self, WgpuRendererError> {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Ningyo WGPU Render Output Texture Creation"),
        });

        let width = config.max_width();
        let height = config.max_height();
        let layers = config.layers();

        let (color_target, color_texture) = match config {
            OutputConfiguration::Surface(surface, _, _) => (
                ColorOutput::Surface,
                surface
                    .get_current_texture()
                    .as_surface_texture()?
                    .texture
                    .texture
                    .clone(),
            ),
            OutputConfiguration::Texture(_, format) => {
                let texture = DeviceTexture::empty_render_target(
                    device,
                    &mut encoder,
                    width,
                    height,
                    layers,
                    *format,
                );
                let out_tex = texture.texture().clone();
                (ColorOutput::Texture(texture), out_tex)
            }
            OutputConfiguration::UserTarget(target, _) => (
                ColorOutput::Texture(target.clone()),
                target.texture().clone(),
            ),
        };
        let stencil_target = DepthStencilTexture::empty_render_target(
            device,
            &mut encoder,
            width,
            height,
            layers,
            wgpu::TextureFormat::Depth24PlusStencil8,
        );
        let composite_target = GBuffer::new(
            device,
            &mut encoder,
            width,
            height,
            layers,
            //TODO: Wait, why? Nothing we work with is HDR.
            wgpu::TextureFormat::Rgba16Float,
            //TODO: You know wgpu has a stencil only format, right?
            wgpu::TextureFormat::Depth24PlusStencil8,
        );
        let mut color_layer_views = vec![];
        let mut stencil_layer_views = vec![];
        let mut composite_albedo_layer_views = vec![];
        let mut composite_emissive_layer_views = vec![];
        let mut composite_bump_layer_views = vec![];
        let mut composite_stencil_layer_views = vec![];

        for layer in 0..layers {
            color_layer_views.push(color_texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: layer,
                array_layer_count: Some(1),
                ..Default::default()
            }));
            stencil_layer_views.push(stencil_target.layer_view(layer));
            composite_albedo_layer_views.push(composite_target.albedo().layer_view(layer));
            composite_emissive_layer_views.push(composite_target.emissive().layer_view(layer));
            composite_bump_layer_views.push(composite_target.bump().layer_view(layer));
            composite_stencil_layer_views.push(composite_target.stencil().layer_view(layer));
        }

        let me = Self {
            color_target,
            stencil_target,
            composite_target,
            color_layer_views,
            stencil_layer_views,
            composite_albedo_layer_views,
            composite_emissive_layer_views,
            composite_bump_layer_views,
            composite_stencil_layer_views,
        };

        queue.submit(std::iter::once(encoder.finish()));

        Ok(me)
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
            OutputConfiguration::Surface(_, _, _) => {
                matches!(self.color_target, ColorOutput::Surface)
            }
            OutputConfiguration::Texture(_, format) => match &self.color_target {
                ColorOutput::Surface => false,
                ColorOutput::Texture(texture) => {
                    self.stencil_target.texture().width() == texture.texture().width()
                        && self.stencil_target.texture().height() == texture.texture().height()
                        && self.stencil_target.texture().depth_or_array_layers()
                            == texture.texture().depth_or_array_layers()
                        && texture.texture().format() == *format
                }
            },
            OutputConfiguration::UserTarget(target, _) => match &self.color_target {
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

    pub fn composite(&self) -> &GBuffer {
        &self.composite_target
    }

    pub fn stencil(&self) -> &DepthStencilTexture {
        &self.stencil_target
    }

    /// Retrieve a view for a single layer of the color target.
    pub fn color_target_layer_view(
        &self,
        layer: usize,
    ) -> Result<&wgpu::TextureView, WgpuRendererError> {
        self.color_layer_views
            .get(layer)
            .ok_or(WgpuRendererError::InvalidViewportId(layer))
    }

    /// Retrieve a view for a single layer of the stencil target.
    pub fn stencil_layer_view(
        &self,
        layer: usize,
    ) -> Result<&wgpu::TextureView, WgpuRendererError> {
        self.stencil_layer_views
            .get(layer)
            .ok_or(WgpuRendererError::InvalidViewportId(layer))
    }

    /// Retrieve a view for a single layer of the composite albedo target.
    pub fn composite_albedo_layer_view(
        &self,
        layer: usize,
    ) -> Result<&wgpu::TextureView, WgpuRendererError> {
        self.composite_albedo_layer_views
            .get(layer)
            .ok_or(WgpuRendererError::InvalidViewportId(layer))
    }

    /// Retrieve a view for a single layer of the composite emissive target.
    pub fn composite_emissive_layer_view(
        &self,
        layer: usize,
    ) -> Result<&wgpu::TextureView, WgpuRendererError> {
        self.composite_emissive_layer_views
            .get(layer)
            .ok_or(WgpuRendererError::InvalidViewportId(layer))
    }

    /// Retrieve a view for a single layer of the composite bump target.
    pub fn composite_bump_layer_view(
        &self,
        layer: usize,
    ) -> Result<&wgpu::TextureView, WgpuRendererError> {
        self.composite_bump_layer_views
            .get(layer)
            .ok_or(WgpuRendererError::InvalidViewportId(layer))
    }

    /// Retrieve a view for a single layer of the composite stencil target.
    pub fn composite_stencil_layer_view(
        &self,
        layer: usize,
    ) -> Result<&wgpu::TextureView, WgpuRendererError> {
        self.composite_stencil_layer_views
            .get(layer)
            .ok_or(WgpuRendererError::InvalidViewportId(layer))
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
            config: OutputConfiguration::Surface(surface, config, Camera::default()),
            viewports_buffer: None,
        }
    }

    pub fn new_texture_target() -> Self {
        RenderTarget {
            // NOTE: Rgba8Unorm is assumed as a "standard" texture type; but
            // this is not guaranteed to be a good format for all use cases.
            config: OutputConfiguration::Texture(vec![], wgpu::TextureFormat::Rgba8Unorm),
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
    /// append a viewport of that size to the list. Thus, you can specify
    /// multiple viewports by resizing each one in order.
    pub fn resize(
        &mut self,
        width: u32,
        height: u32,
        viewport: usize,
    ) -> Result<(), WgpuRendererError> {
        if width > 0 && height > 0 {
            match &mut self.config {
                OutputConfiguration::Surface(_, config, _) => {
                    config.width = width;
                    config.height = height;
                }
                OutputConfiguration::Texture(viewports, _) => {
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
                OutputConfiguration::UserTarget(_, _) => {}
            }

            Ok(())
        } else {
            Err(WgpuRendererError::SizeCannotBeZero)
        }
    }

    /// Retrieve a given viewport's camera structure.
    ///
    /// Each viewport has an independent camera that specifies a particular
    /// position, scale, and rotation for that view, independent of any other
    /// transformations applied to individual renderers.
    ///
    /// Surface and user-provided render targets have one fixed viewport in
    /// slot 0. For renderer-allocated targets, any viewport that has been
    /// previously defined by a call to `resize` may have its camera altered.
    pub fn viewport_camera(&self, viewport: usize) -> Option<&Camera> {
        match &self.config {
            OutputConfiguration::Surface(_, _, _) => None,
            OutputConfiguration::UserTarget(_, camera) => Some(camera),
            OutputConfiguration::Texture(viewports, _) => {
                viewports.get(viewport).map(|vp| &vp.view)
            }
        }
    }

    /// Retrieve a given viewport's camera structure for mutation.
    ///
    /// Each viewport has an independent camera that specifies a particular
    /// position, scale, and rotation for that view, independent of any other
    /// transformations applied to individual renderers.
    ///
    /// Surface and user-provided render targets have one fixed viewport in
    /// slot 0. For renderer-allocated targets, any viewport that has been
    /// previously defined by a call to `resize` may have its camera altered.
    pub fn viewport_camera_mut(&mut self, viewport: usize) -> Option<&mut Camera> {
        match &mut self.config {
            OutputConfiguration::Surface(_, _, _) => None,
            OutputConfiguration::UserTarget(_, camera) => Some(camera),
            OutputConfiguration::Texture(viewports, _) => {
                viewports.get_mut(viewport).map(|vp| &mut vp.view)
            }
        }
    }

    /// Get the position and size of a given viewport.
    pub fn viewport_origin_and_extent(
        &self,
        viewport: usize,
    ) -> Option<(wgpu::Origin3d, wgpu::Extent3d)> {
        self.config.as_origin_and_extent(viewport)
    }

    /// Provide a user-specified texture as a render target.
    ///
    /// The texture must have been created with the texture usages in
    /// `required_render_target_uses` and must originate from the same device
    /// that we are using to render with.
    pub fn set_render_target(&mut self, target: wgpu::Texture) -> Result<(), WgpuRendererError> {
        let camera = self.viewport_camera(0).cloned().unwrap_or_default();
        self.config =
            OutputConfiguration::UserTarget(DeviceTexture::user_render_target(target)?, camera);

        Ok(())
    }

    /// Apply prior configuration changes.
    pub fn apply(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), WgpuRendererError> {
        if self.outputs.is_none()
            || !self
                .outputs
                .as_ref()
                .unwrap()
                .is_compatible_with_configuration(&self.config)
        {
            self.outputs = Some(RenderOutput::new(device, queue, &self.config)?);
        }

        //TODO: We should have a flag to check if the viewport configuration
        //was actually mutated or not.

        let viewports_config = self.config.as_viewport_config(self.outputs.as_ref());
        let viewports_config = Viewports {
            viewports: viewports_config,
        };

        let mut data = vec![0; viewports_config.required_size()];
        viewports_config.write_buffer(&mut data[..]);

        if let Some(buffer) = self.viewports_buffer.as_ref() {
            if buffer.size() >= viewports_config.required_size() as u64 {
                queue.write_buffer(buffer, 0, &data[..]);
                return Ok(());
            }
        }

        self.viewports_buffer = Some(device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Viewport configuration buffer init"),
                contents: &data[..],
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            },
        ));

        Ok(())
    }

    /// Retrieve the current surface, if surface rendering is enabled.
    pub fn surface(&self) -> Option<&wgpu::Surface<'surf>> {
        match &self.config {
            OutputConfiguration::Surface(surface, _, _) => Some(surface),
            _ => None,
        }
    }

    /// Get the color target view for rendering.
    pub fn color_target(&self) -> Result<wgpu::Texture, WgpuRendererError> {
        match &self.config {
            OutputConfiguration::Surface(surface, _, _) => Ok(surface
                .get_current_texture()
                .as_surface_texture()?
                .texture
                .texture
                .clone()),
            OutputConfiguration::UserTarget(target, _) => Ok(target.texture().clone()),
            OutputConfiguration::Texture(_, _) => {
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
            OutputConfiguration::Surface(surface, _, _) => Ok(surface
                .get_current_texture()
                .as_surface_texture()?
                .texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default())),
            OutputConfiguration::UserTarget(target, _) => Ok(target.view().clone()),
            OutputConfiguration::Texture(_, _) => {
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

    pub fn color_target_format(&self) -> wgpu::TextureFormat {
        match &self.config {
            OutputConfiguration::Surface(_, config, _) => config.format,
            OutputConfiguration::Texture(_, format) => *format,
            OutputConfiguration::UserTarget(texture, _) => texture.texture().format(),
        }
    }

    /// Set the color format that this renders to.
    ///
    /// Note that this has no effect for user-specified texture targets (see
    /// `set_render_target`); you must allocate and provide a different texture
    /// in that case.
    pub fn set_color_target_format(&mut self, format: wgpu::TextureFormat) {
        match &mut self.config {
            OutputConfiguration::Surface(_, config, _) => {
                config.format = format;
            }
            OutputConfiguration::Texture(_, my_format) => {
                *my_format = format;
            }
            OutputConfiguration::UserTarget(_, _) => {}
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

    /// Get the current set of output textures.
    pub fn outputs(&self) -> Result<&RenderOutput, WgpuRendererError> {
        self.outputs
            .as_ref()
            .ok_or(WgpuRendererError::ViewportNotInitialized)
    }

    /// Convenience method for presenting the rendered surface.
    ///
    /// Does nothing if this renderer is not directly rendering to a surface.
    pub fn present(&self) -> Result<(), ningyo_extensions::SurfaceError> {
        if let Some(surface) = self.surface() {
            surface
                .get_current_texture()
                .as_surface_texture()?
                .texture
                .present();
        }

        Ok(())
    }

    /// Issue a clear command on the output color buffer.
    pub fn clear(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<wgpu::SubmissionIndex, WgpuRendererError> {
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

        Ok(queue.submit(std::iter::once(encoder.finish())))
    }

    /// Issues a copy command for the given viewport to a target texture.
    ///
    /// The given target texture must be the same size or larger than the
    /// selected viewport, and of the same texture format. If the target is not
    /// large enough to receive the copy, the copied pixels will be cropped to
    /// fit.
    pub fn copy(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::Texture,
        viewport: usize,
    ) -> Result<wgpu::SubmissionIndex, WgpuRendererError> {
        let (origin, extent) = self
            .viewport_origin_and_extent(viewport)
            .ok_or(WgpuRendererError::ViewportNotInitialized)?;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Output copy"),
        });

        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.color_target()?,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: min(extent.width, target.width()),
                height: min(extent.height, target.height()),
                depth_or_array_layers: 1,
            },
        );

        Ok(queue.submit(std::iter::once(encoder.finish())))
    }

    /// Get the buffer containing the viewport configuration data for shaders.
    pub fn viewports_config(&self) -> Result<&wgpu::Buffer, WgpuRendererError> {
        self.viewports_buffer
            .as_ref()
            .ok_or(WgpuRendererError::ViewportNotInitialized)
    }
}
