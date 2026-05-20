use glam::UVec2;
use inox2d::math::camera::Camera;
use inox2d::model::Model;
//hey wait a second that's just a u32 newtype! UUIDs are four of those!
use inox2d::render::InoxRenderer;
use ningyo_extensions::CurrentSurfaceTextureExt;
use std::error::Error;
use std::sync::{Arc, Mutex};
use wgpu;

use crate::buffer_builder::BufferBuilder;
use crate::shaders::basic::{basic_frag, basic_mask_frag, basic_vert, composite_frag};
use crate::texture::{DepthStencilTexture, DeviceTexture, GBuffer};

use std::collections::HashMap;

use crate::draw_session::WgpuDrawSession;
use crate::error::WgpuRendererError;
use crate::resources::WgpuResources;
use crate::uploads::WgpuUploads;

/// Buffer offsets for all four of our main shaders.
///
/// Whenever a node is prepassed, we set all of the relevant buffer offsets
/// here. There is one of this struct for each node in the puppet.
#[derive(Default)]
pub struct BufferIndices {
    pub(crate) basic_vert: Option<usize>,
    pub(crate) basic_frag: Option<usize>,
    pub(crate) basic_mask_frag: Option<usize>,
    pub(crate) composite_frag: Option<usize>,
}

impl BufferIndices {
    pub fn clear(&mut self) {
        self.basic_vert = None;
        self.basic_frag = None;
        self.basic_mask_frag = None;
        self.composite_frag = None;
    }
}

pub struct WgpuRenderer<'window> {
    pub(crate) surface: Option<(wgpu::Surface<'window>, wgpu::SurfaceConfiguration)>,
    pub(crate) target: (Option<DeviceTexture>, UVec2),

    /// All textures used as render targets, excluding the surface color
    /// buffer.
    ///
    /// GBuffer is used solely for composite rendering, where rendered pixels
    /// are used for a deferred shading pass.
    pub(crate) render_targets: Option<(GBuffer, DepthStencilTexture)>,

    /// Where to draw the puppet relative to the current target surface or
    /// texture.
    pub camera: Camera,

    /// Builder for basic part vertex uniforms
    pub(crate) builder_basic_vert: BufferBuilder<basic_vert::Input>,

    /// Builder for basic part frag uniforms
    pub(crate) builder_basic_frag: BufferBuilder<basic_frag::Input>,

    /// Builder for basic mask frag uniforms
    pub(crate) builder_basic_mask_frag: BufferBuilder<basic_mask_frag::Input>,

    /// Builder for composite frag uniforms
    pub(crate) builder_composite_frag: BufferBuilder<composite_frag::Input>,

    /// An index into the buffers for each node.
    pub(crate) buffer_indices: HashMap<u32, BufferIndices>,

    pub(crate) uploads: WgpuUploads,
    pub(crate) resources: Arc<Mutex<WgpuResources>>,

    /// The device to render to.
    ///
    /// Must match the device in WgpuResources (this is a cache to avoid lock
    /// contention)
    device: wgpu::Device,

    /// The queue to render to.
    ///
    /// Must match the device in WgpuResources (this is a cache to avoid lock
    /// contention)
    queue: wgpu::Queue,
}

impl<'window> WgpuRenderer<'window> {
    /// Create a WGPU renderer that presents a surface after rendering
    /// completes.
    ///
    /// This is primarily intended for demo apps that do not need to share
    /// access to the rendering hardware. For real work, you likely want to
    /// create your own renderer and draw to a texture (see `new_headless`).
    ///
    /// In this mode, WgpuRenderer creates its own instance and adapter to
    /// guarantee compatibility with the given surface.
    pub async fn new_with_surface(
        target: impl Into<wgpu::SurfaceTarget<'window>>,
        model: &Model,
    ) -> Result<Self, WgpuRendererError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let surface = instance.create_surface(target)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await?;

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

        let resources = Arc::new(Mutex::new(WgpuResources::new(&adapter).await?));

        let mut renderer = Self::new_headless_with_resources(resources, model)?;

        renderer.surface = Some((surface, config));

        Ok(renderer)
    }

    /// Create a WGPU renderer that renders to an internal texture.
    pub async fn new_headless(model: &Model) -> Result<Self, WgpuRendererError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                ..Default::default()
            })
            .await?;
        let resources = Arc::new(Mutex::new(WgpuResources::new(&adapter).await?));

        // We actually can't create our render target until we know our size.

        Ok(Self::new_headless_with_resources(resources, model)?)
    }

    /// Create a renderer with a user-specified resource pack.
    pub fn new_headless_with_resources(
        resources_arc: Arc<Mutex<WgpuResources>>,
        model: &Model,
    ) -> Result<Self, WgpuRendererError> {
        let mut resources = resources_arc.lock().unwrap();
        let device = resources.device.clone();
        let queue = resources.queue.clone();

        let uploads = WgpuUploads::new(model, &mut *resources)?;
        drop(resources);

        Ok(WgpuRenderer {
            surface: None,

            // The 640x480 size is a placeholder, we're waiting for a resize.
            target: (None, UVec2::new(640, 480)),
            camera: Camera::default(),
            render_targets: None,
            uploads,
            resources: resources_arc,
            builder_basic_frag: BufferBuilder::new(wgpu::Limits::default()),
            builder_basic_mask_frag: BufferBuilder::new(wgpu::Limits::default()),
            builder_basic_vert: BufferBuilder::new(wgpu::Limits::default()),
            builder_composite_frag: BufferBuilder::new(wgpu::Limits::default()),
            buffer_indices: Default::default(),

            device,
            queue,
        })
    }

    /// Indicate to the renderer that the target of rendering has changed size.
    ///
    /// If this renderer was created to render directly to a surface, the
    /// surface will be reconfigured. Otherwise, this renderer will allocate a
    /// new texture of the required size.
    ///
    /// If you wish to provide your own target textures, do not call this
    /// function. Instead, call `resize_with_texture`.
    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), WgpuRendererError> {
        if width > 0 && height > 0 {
            let old_size = if let Some((_, config)) = &self.surface {
                Some((config.width, config.height))
            } else if let Some(target) = &self.target.0 {
                Some((target.texture().width(), target.texture().height()))
            } else {
                None
            };

            if let Some((old_width, old_height)) = old_size {
                if old_width == width && old_height == height && self.render_targets.is_some() {
                    //We don't need to do anything.
                    return Ok(());
                }
            }

            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Inox2D texture resizes"),
                });

            if let Some((surface, config)) = &mut self.surface {
                config.width = width;
                config.height = height;
                surface.configure(&self.device, config);
            } else if self.target.0.is_none() {
                panic!("Render target texture must have been set before resize!!!")
            }

            self.render_targets = Some((
                GBuffer::new(
                    &self.device,
                    &mut encoder,
                    width,
                    height,
                    //TODO: Wait, why? Nothing we work with is HDR.
                    wgpu::TextureFormat::Rgba16Float,
                    //TODO: You know wgpu has a stencil only format, right?
                    wgpu::TextureFormat::Depth24PlusStencil8,
                ),
                DepthStencilTexture::empty_render_target(
                    &self.device,
                    &mut encoder,
                    width,
                    height,
                    wgpu::TextureFormat::Depth24PlusStencil8,
                ),
            ));

            self.queue.submit(std::iter::once(encoder.finish()));
            Ok(())
        } else {
            Err(WgpuRendererError::SizeCannotBeZero)
        }
    }

    pub fn required_render_target_uses() -> wgpu::TextureUsages {
        DeviceTexture::required_render_target_uses()
    }

    /// Provide a user-specified texture as a render target.
    ///
    /// The texture must have been created with the texture usages in
    /// `required_render_target_uses` and must originate from the same device
    /// that we are using to render with.
    pub fn set_render_target(&mut self, target: wgpu::Texture) -> Result<(), WgpuRendererError> {
        let width = target.width();
        let height = target.height();
        let new_target = DeviceTexture::user_render_target(target)?;

        self.target.1 = UVec2::new(new_target.texture().width(), new_target.texture().height());
        self.target.0 = Some(new_target);

        self.resize(width, height)
    }

    /// Convenience method for presenting the rendered surface.
    ///
    /// Does nothing if this renderer is not directly rendering to a surface.
    pub fn present(&self) -> Result<(), ningyo_extensions::SurfaceError> {
        if let Some((surface, _config)) = &self.surface {
            surface
                .get_current_texture()
                .as_surface_texture()?
                .texture
                .present();
        }

        Ok(())
    }

    /// Convenience method for clearing the target texture or surface.
    pub fn clear(&self) -> Result<(), ningyo_extensions::SurfaceError> {
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("WGPURenderer::clear"),
            });

        match (&self.surface, &self.target) {
            (Some((surface, _)), (None, _)) => {
                encoder.clear_texture(
                    &surface
                        .get_current_texture()
                        .as_surface_texture()?
                        .texture
                        .texture,
                    &wgpu::ImageSubresourceRange {
                        aspect: wgpu::TextureAspect::All,
                        base_mip_level: 0,
                        mip_level_count: None,
                        base_array_layer: 0,
                        array_layer_count: None,
                    },
                );
            }
            (None, (Some(target), _)) => target.clear(&mut encoder),
            _ => {}
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    pub fn device(&self) -> wgpu::Device {
        self.device.clone()
    }

    pub fn target_texture(&self) -> Option<wgpu::Texture> {
        if let (Some(targ), _) = &self.target {
            return Some(targ.texture().clone());
        }

        None
    }
}

impl<'window> InoxRenderer for WgpuRenderer<'window> {
    type Draw<'a>
        = WgpuDrawSession<'a>
    where
        Self: 'a;

    fn on_begin_draw<'a>(
        &'a mut self,
        puppet: &inox2d::puppet::Puppet,
    ) -> Result<Self::Draw<'a>, Box<dyn Error>> {
        WgpuDrawSession::begin(self, puppet)
    }
}
