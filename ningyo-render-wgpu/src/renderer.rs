use inox2d::math::camera::Camera;
use inox2d::model::Model;
//hey wait a second that's just a u32 newtype! UUIDs are four of those!
use inox2d::render::InoxRenderer;
use std::error::Error;
use std::sync::{Arc, Mutex};
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
    pub(crate) render_target: Arc<Mutex<RenderTarget<'window>>>,

    /// Where to draw the puppet relative to the artboard.
    pub camera: Camera,

    /// Builder for basic part vertex uniforms
    pub(crate) builder_basic_vert: BufferBuilder<basic_vert::InputArray>,

    /// Builder for basic part frag uniforms
    pub(crate) builder_basic_frag: BufferBuilder<basic_frag::InputArray>,

    /// Builder for basic mask frag uniforms
    pub(crate) builder_basic_mask_frag: BufferBuilder<basic_mask_frag::InputArray>,

    /// Builder for composite frag uniforms
    pub(crate) builder_composite_frag: BufferBuilder<composite_frag::Input>,

    /// Builder for indirect rendering buffer
    pub(crate) builder_indirect: BufferBuilder<wgpu::util::DrawIndexedIndirectArgs>,

    /// An index into the buffers for each node.
    pub(crate) buffer_indices: HashMap<u32, BufferIndices>,

    pub(crate) bind_cache: BindingCache<'static>,

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
        let target = Arc::new(Mutex::new(RenderTarget::new_with_surface(
            surface, &adapter,
        )));

        let renderer = Self::new_headless_with_resources(resources, model, target)?;

        Ok(renderer)
    }

    /// Create a WGPU renderer that renders to an internal texture.
    pub async fn new_headless(
        model: &Model,
        target: Arc<Mutex<RenderTarget<'window>>>,
    ) -> Result<Self, WgpuRendererError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                ..Default::default()
            })
            .await?;
        let resources = Arc::new(WgpuResources::new(&adapter).await?);

        Ok(Self::new_headless_with_resources(resources, model, target)?)
    }

    /// Create a renderer with a user-specified resource pack.
    pub fn new_headless_with_resources(
        resources: Arc<WgpuResources>,
        model: &Model,
        target: Arc<Mutex<RenderTarget<'window>>>,
    ) -> Result<Self, WgpuRendererError> {
        let device = resources.device.clone();

        let uploads = WgpuUploads::new(model, &resources)?;

        Ok(WgpuRenderer {
            camera: Camera::default(),
            render_target: target,
            uploads,
            resources,
            builder_basic_frag: BufferBuilder::new_array_builder::<basic_frag::InputArray>(
                wgpu::BufferUsages::STORAGE,
            ),
            builder_basic_mask_frag: BufferBuilder::new_array_builder::<basic_mask_frag::InputArray>(
                wgpu::BufferUsages::STORAGE,
            ),
            builder_basic_vert: BufferBuilder::new_array_builder::<basic_vert::InputArray>(
                wgpu::BufferUsages::STORAGE,
            ),
            builder_composite_frag: BufferBuilder::new(
                wgpu::Limits::default(),
                wgpu::BufferUsages::UNIFORM,
            ),
            builder_indirect: BufferBuilder::new_array_builder::<wgpu::util::DrawIndexedIndirectArgs>(
                wgpu::BufferUsages::INDIRECT,
            ),
            buffer_indices: Default::default(),
            draw_commands: Default::default(),
            bind_cache: Default::default(),
            last_submission_index: None,
            device,
        })
    }

    pub fn required_render_target_uses() -> wgpu::TextureUsages {
        DeviceTexture::required_render_target_uses()
    }

    pub fn device(&self) -> wgpu::Device {
        self.device.clone()
    }

    pub fn last_submission_index(&self) -> Option<wgpu::SubmissionIndex> {
        self.last_submission_index.clone()
    }
}

impl<'window> InoxRenderer for WgpuRenderer<'window> {
    type Draw<'a>
        = WgpuDrawSession<'a, 'window>
    where
        Self: 'a;

    fn on_begin_draw<'a>(
        &'a mut self,
        puppet: &inox2d::puppet::Puppet,
    ) -> Result<Self::Draw<'a>, Box<dyn Error>> {
        WgpuDrawSession::begin(self, puppet)
    }
}
