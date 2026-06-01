use inox2d::math::camera::Camera;
use inox2d::model::Model;
//hey wait a second that's just a u32 newtype! UUIDs are four of those!
use inox2d::render::InoxRenderer;
use ningyo_extensions::CurrentSurfaceTextureExt;
use std::error::Error;
use std::sync::Arc;
use wgpu;

use crate::buffer_builder::BufferBuilder;
use crate::shaders::basic::{basic_frag, basic_mask_frag, basic_vert, composite_frag};
use crate::texture::DeviceTexture;

use std::collections::HashMap;

use crate::binding_cache::BindingCache;
use crate::draw_command::DrawCommandList;
use crate::draw_session::WgpuDrawSession;
use crate::error::WgpuRendererError;
use crate::resources::WgpuResources;
use crate::targets::RenderTarget;
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
    /// The target of all rendering operations.
    pub(crate) render_target: RenderTarget<'window>,

    /// Where to draw the puppet relative to the artboard.
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

    pub(crate) bind_cache: BindingCache,

    /// Static resources common to all renderers rendering the same puppet.
    pub(crate) uploads: WgpuUploads,

    /// Static resources common to all renderers regardless of puppet.
    pub(crate) resources: Arc<WgpuResources>,

    /// All the current drawing commands.
    ///
    /// We store them here because wgpu basically requires render passes to
    /// live on one stack frame. No, `forget_lifetime()` doesn't work, that
    /// runs into weird locking bugs where the command encoder is just
    /// permenantly poisoned.
    pub(crate) draw_commands: DrawCommandList,

    /// The last submission index received when queueing our work.
    pub(crate) last_submission_index: Option<wgpu::SubmissionIndex>,

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

        let resources = Arc::new(WgpuResources::new(&adapter).await?);
        let target = RenderTarget::new_with_surface(surface, &adapter);

        let renderer = Self::new_headless_internal(resources, model, target)?;

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
        let resources = Arc::new(WgpuResources::new(&adapter).await?);
        let target = RenderTarget::new_texture_target();

        Ok(Self::new_headless_internal(resources, model, target)?)
    }

    /// Create a renderer with a user-specified resource pack.
    pub fn new_headless_with_resources(
        resources: Arc<WgpuResources>,
        model: &Model,
    ) -> Result<Self, WgpuRendererError> {
        let target = RenderTarget::new_texture_target();

        Ok(Self::new_headless_internal(resources, model, target)?)
    }

    fn new_headless_internal(
        resources: Arc<WgpuResources>,
        model: &Model,
        target: RenderTarget<'window>,
    ) -> Result<Self, WgpuRendererError> {
        let device = resources.device.clone();
        let queue = resources.queue.clone();

        let uploads = WgpuUploads::new(model, &resources)?;

        Ok(WgpuRenderer {
            camera: Camera::default(),
            render_target: target,
            uploads,
            resources,
            builder_basic_frag: BufferBuilder::new(wgpu::Limits::default()),
            builder_basic_mask_frag: BufferBuilder::new(wgpu::Limits::default()),
            builder_basic_vert: BufferBuilder::new(wgpu::Limits::default()),
            builder_composite_frag: BufferBuilder::new(wgpu::Limits::default()),
            buffer_indices: Default::default(),
            draw_commands: Default::default(),
            bind_cache: Default::default(),
            last_submission_index: None,
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
            self.render_target.resize(width, height);
            self.render_target.apply(&self.device, &self.queue);

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
        self.render_target.set_render_target(target)?;
        self.render_target.apply(&self.device, &self.queue);

        Ok(())
    }

    /// Convenience method for presenting the rendered surface.
    ///
    /// Does nothing if this renderer is not directly rendering to a surface.
    pub fn present(&self) -> Result<(), ningyo_extensions::SurfaceError> {
        if let Some(surface) = self.render_target.surface() {
            surface
                .get_current_texture()
                .as_surface_texture()?
                .texture
                .present();
        }

        Ok(())
    }

    /// Convenience method for clearing the target texture or surface.
    pub fn clear(&self) -> Result<(), WgpuRendererError> {
        self.render_target.clear(&self.device, &self.queue)
    }

    pub fn device(&self) -> wgpu::Device {
        self.device.clone()
    }

    pub fn target_texture(&self) -> Result<wgpu::Texture, WgpuRendererError> {
        self.render_target.color_target()
    }

    pub fn last_submission_index(&self) -> Option<wgpu::SubmissionIndex> {
        self.last_submission_index.clone()
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
