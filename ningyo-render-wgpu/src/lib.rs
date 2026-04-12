use glam::Mat4;
use glam::UVec2;
use inox2d::math::camera::Camera;
use inox2d::model::Model;
use inox2d::node::{InoxNodeUuid, components, drawables}; //hey wait a second that's just a u32 newtype! UUIDs are four of those!
use inox2d::render::{self, DrawSession, InoxRenderer};
use inox2d::texture::decode_model_textures;
use std::error::Error;
use std::sync::{Arc, Mutex};
use wgpu;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

mod pipeline;
mod shader;
mod shaders;
mod texture;

use crate::texture::{DepthStencilTexture, DeviceTexture, GBuffer};
use shader::UniformBlock;
use shaders::basic::{
    basic_frag, basic_mask_frag, basic_vert, composite_frag, composite_mask_frag, composite_vert,
};

use std::collections::HashMap;

use ningyo_gtk_wgpu::prelude::*;

/// Cast Vec2 to array.
///
/// SAFETY: This inherits the safety considerations of glam's own
/// `upload_array_to_gl`. Specifically, we rely on the fact that it's own Vec2
/// struct is plain-ol-data and we're only working with immutables.
///
/// NOTE: At some point, rewrite inox2D's vertex arrays to use bytemuck and a
/// custom Vec2 struct.
pub fn cast_vec2(array: &[glam::Vec2]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(array.as_ptr() as *const u8, std::mem::size_of_val(array)) }
}

/// Cast u16s to array.
///
/// SAFETY: This inherits the safety considerations of glam's own
/// `upload_array_to_gl`.
///
/// NOTE: This probably can already be bytemucked
pub fn cast_index(array: &[u32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(array.as_ptr() as *const u8, std::mem::size_of_val(array)) }
}

#[derive(Debug, thiserror::Error)]
#[error("Could not initialize wgpu renderer: {0}")]
pub enum WgpuRendererError {
    InstanceError(#[from] wgpu_hal::InstanceError),
    CreateSurfaceError(#[from] wgpu::CreateSurfaceError),
    RequestAdapterError(#[from] wgpu::RequestAdapterError),
    RequestDeviceError(#[from] wgpu::RequestDeviceError),
    SurfaceError(#[from] wgpu::SurfaceError),

    #[error("Model rendering not initialized")]
    ModelRenderingNotInitialized,

    #[error("Size cannot be zero")]
    SizeCannotBeZero,
}

/// WGPU resources that are invariant to the current puppet being rendered.
///
/// Multiple renderes may share resources so long as they use the same device
/// and queue.
///
/// It is recommended to shove this in an Arc<Mutex<>>.
struct WgpuResources {
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,

    part_shader_vert: basic_vert::Shader,
    part_shader_frag: basic_frag::Shader,
    part_shader_mask_frag: basic_mask_frag::Shader,

    masked_depthstencil: wgpu::DepthStencilState,
    mask_depthstencil: wgpu::DepthStencilState,

    part_pipeline: pipeline::PipelineGroup<basic_vert::Shader, basic_frag::Shader>,
    part_mask_pipeline: pipeline::PipelineGroup<basic_vert::Shader, basic_mask_frag::Shader>,

    composite_shader_vert: composite_vert::Shader,
    composite_shader_frag: composite_frag::Shader,
    _composite_shader_mask_frag: composite_mask_frag::Shader,

    composite_pipeline: pipeline::PipelineGroup<composite_vert::Shader, composite_frag::Shader>,
    _composite_mask_pipeline:
        pipeline::PipelineGroup<composite_vert::Shader, composite_mask_frag::Shader>,
}

impl WgpuResources {
    async fn new(adapter: &wgpu::Adapter) -> Result<Self, WgpuRendererError> {
        let (device, queue) = adapter
            .request_device_with_extensions(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER
                    | wgpu::Features::CLEAR_TEXTURE
                    | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                    | wgpu::Features::DEPTH_CLIP_CONTROL,
                required_limits: wgpu::Limits {
                    max_color_attachment_bytes_per_sample: 48,
                    ..Default::default()
                },
                ..Default::default()
            })
            .await?;

        // Compile all our shaders now.
        let part_shader_vert = basic_vert::Shader::new(&device);
        let part_shader_frag = basic_frag::Shader::new(&device);
        let part_shader_mask_frag = basic_mask_frag::Shader::new(&device);

        let masked_depthstencil = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::Always,
            stencil: wgpu::StencilState {
                front: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Equal,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                back: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Equal,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                read_mask: 0xFF,
                write_mask: 0x00,
            },
            bias: wgpu::DepthBiasState::default(),
        };

        let mask_depthstencil = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::Always,
            stencil: wgpu::StencilState {
                front: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                back: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                read_mask: 0xFF,
                write_mask: 0xFF,
            },
            bias: wgpu::DepthBiasState::default(),
        };

        //TODO: We need a pipeline per Inochi blending mode
        //(or some kind of ubershader blending)

        let part_pipeline =
            pipeline::PipelineGroup::new(part_shader_vert.clone(), part_shader_frag.clone());
        let part_mask_pipeline =
            pipeline::PipelineGroup::new(part_shader_vert.clone(), part_shader_mask_frag.clone());

        let composite_shader_vert = composite_vert::Shader::new(&device);
        let composite_shader_frag = composite_frag::Shader::new(&device);
        let composite_shader_mask_frag = composite_mask_frag::Shader::new(&device);

        let composite_pipeline = pipeline::PipelineGroup::new(
            composite_shader_vert.clone(),
            composite_shader_frag.clone(),
        );
        let composite_mask_pipeline = pipeline::PipelineGroup::new(
            composite_shader_vert.clone(),
            composite_shader_mask_frag.clone(),
        );

        // Flush all pending work.
        // In wgpu, texture uploads etc will only execute at submit time
        queue.submit([]);

        Ok(WgpuResources {
            adapter: adapter.clone(),
            device,
            queue,
            part_shader_vert,
            part_shader_frag,
            part_shader_mask_frag,
            mask_depthstencil,
            masked_depthstencil,
            part_pipeline,
            part_mask_pipeline,
            composite_shader_vert,
            composite_shader_frag,
            _composite_shader_mask_frag: composite_shader_mask_frag,
            composite_pipeline,
            _composite_mask_pipeline: composite_mask_pipeline,
        })
    }
}

/// All of the puppet-specific textures for this renderer.
///
/// Multiple renderers may share a puppet (e.g. to render to different views)
struct WgpuUploads {
    verts: wgpu::Buffer,
    uvs: wgpu::Buffer,
    deforms: wgpu::Buffer,
    indices: wgpu::Buffer,

    model_textures: Vec<DeviceTexture>,
    model_sampler: wgpu::Sampler,
}

pub struct WgpuRenderer<'window> {
    surface: Option<(wgpu::Surface<'window>, wgpu::SurfaceConfiguration)>,
    target: Arc<Mutex<(Option<DeviceTexture>, UVec2)>>,

    /// All textures used as render targets, excluding the surface color
    /// buffer.
    ///
    /// GBuffer is used solely for composite rendering, where rendered pixels
    /// are used for a deferred shading pass.
    render_targets: Option<(GBuffer, DepthStencilTexture)>,

    /// Where to draw the puppet relative to the current target surface or
    /// texture.
    pub camera: Camera,

    uploads: WgpuUploads,
    resources: Arc<Mutex<WgpuResources>>,
}

impl<'window> WgpuRenderer<'window> {
    /// Create a WGPU renderer that presents a surface after rendering
    /// completes.
    pub async fn new_with_surface(
        target: impl Into<wgpu::SurfaceTarget<'window>>,
        model: &Model,
    ) -> Result<Self, WgpuRendererError> {
        let instance =
            wgpu::Instance::new_with_extensions(wgpu::InstanceDescriptor::from_env_or_default())?;
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

        let mut renderer = Self::new_internal(resources, model)?;

        renderer.surface = Some((surface, config));

        Ok(renderer)
    }

    /// Create a WGPU renderer that renders to an internal texture.
    pub async fn new_headless(model: &Model) -> Result<Self, WgpuRendererError> {
        let instance =
            wgpu::Instance::new_with_extensions(wgpu::InstanceDescriptor::from_env_or_default())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                ..Default::default()
            })
            .await?;
        let resources = Arc::new(Mutex::new(WgpuResources::new(&adapter).await?));

        // We actually can't create our render target until we know our size.

        Ok(Self::new_internal(resources, model)?)
    }

    /// Create a new WGPU renderer using the same device to render a different
    /// puppet.
    ///
    /// The surface or texture render target will be cloned too.
    pub fn clone_with_model(&self, model: &Model) -> Result<Self, WgpuRendererError> {
        let mut the_clone = Self::new_internal(self.resources.clone(), model)?;

        the_clone.target = self.target.clone();

        Ok(the_clone)
    }

    fn new_internal(
        resources_arc: Arc<Mutex<WgpuResources>>,
        model: &Model,
    ) -> Result<Self, WgpuRendererError> {
        let resources = resources_arc.lock().unwrap();
        let device = &resources.device;
        let queue = &resources.queue;
        let inox_buffers = model
            .puppet
            .render_ctx
            .as_ref()
            .ok_or(WgpuRendererError::ModelRenderingNotInitialized)?;
        //TODO: Change inox2d upstream to use a bytemuck-able array
        let verts = device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!(
                "Inox2D {}::Verts",
                model
                    .puppet
                    .meta
                    .name
                    .as_deref()
                    .unwrap_or("<NAME NOT SPECIFIED>")
            )),
            contents: cast_vec2(inox_buffers.vertex_buffers.verts.as_slice()),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let uvs = device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!(
                "Inox2D {}::UVs",
                model
                    .puppet
                    .meta
                    .name
                    .as_deref()
                    .unwrap_or("<NAME NOT SPECIFIED>")
            )),
            contents: cast_vec2(inox_buffers.vertex_buffers.uvs.as_slice()),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let deforms = device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!(
                "Inox2D {}::Deforms",
                model
                    .puppet
                    .meta
                    .name
                    .as_deref()
                    .unwrap_or("<NAME NOT SPECIFIED>")
            )),
            contents: cast_vec2(inox_buffers.vertex_buffers.deforms.as_slice()),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let indices = device.create_buffer_init(&BufferInitDescriptor {
            label: Some(&format!(
                "Inox2D {}::Indices",
                model
                    .puppet
                    .meta
                    .name
                    .as_deref()
                    .unwrap_or("<NAME NOT SPECIFIED>")
            )),
            contents: cast_index(inox_buffers.vertex_buffers.indices.as_slice()),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        });

        let decoded_textures = decode_model_textures(model.textures.iter());
        let mut texture_handles = vec![];
        for (index, texture) in decoded_textures.iter().enumerate() {
            texture_handles.push(DeviceTexture::new_from_model(
                &device, &queue, model, index, texture,
            ));
        }

        let model_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToBorder,
            address_mode_v: wgpu::AddressMode::ClampToBorder,
            address_mode_w: wgpu::AddressMode::ClampToBorder,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        drop(resources);

        Ok(WgpuRenderer {
            surface: None,

            // The 640x480 size is a placeholder, we're waiting for a resize.
            target: Arc::new(Mutex::new((None, UVec2::new(640, 480)))),
            camera: Camera::default(),
            render_targets: None,
            uploads: WgpuUploads {
                verts,
                uvs,
                deforms,
                indices,
                model_textures: texture_handles,
                model_sampler,
            },
            resources: resources_arc,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), WgpuRendererError> {
        if width > 0 && height > 0 {
            let resources = self.resources.lock().unwrap();

            if let Some((surface, config)) = &mut self.surface {
                config.width = width;
                config.height = height;
                surface.configure(&resources.device, config);
            } else {
                *self.target.lock().unwrap() = (
                    Some(DeviceTexture::empty_render_target_exportable(
                        &resources.adapter,
                        &resources.device,
                        &resources.queue,
                        width,
                        height,
                        wgpu::TextureFormat::Rgba8Unorm,
                    )),
                    UVec2::new(width, height),
                );
            }
            let mut encoder =
                resources
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Inox2D texture resizes"),
                    });

            self.render_targets = Some((
                GBuffer::new(
                    &resources.device,
                    &mut encoder,
                    width,
                    height,
                    //TODO: Wait, why? Nothing we work with is HDR.
                    wgpu::TextureFormat::Rgba16Float,
                    //TODO: You know wgpu has a stencil only format, right?
                    wgpu::TextureFormat::Depth24PlusStencil8,
                ),
                DepthStencilTexture::empty_render_target(
                    &resources.device,
                    &mut encoder,
                    width,
                    height,
                    wgpu::TextureFormat::Depth24PlusStencil8,
                ),
            ));

            resources.queue.submit(std::iter::once(encoder.finish()));
            Ok(())
        } else {
            Err(WgpuRendererError::SizeCannotBeZero)
        }
    }

    fn textures_for_part(
        &self,
        part: &components::TexturedMesh,
    ) -> (&DeviceTexture, &DeviceTexture, &DeviceTexture) {
        (
            &self.uploads.model_textures[part.tex_albedo.raw()],
            &self.uploads.model_textures[part.tex_bumpmap.raw()],
            &self.uploads.model_textures[part.tex_emissive.raw()],
        )
    }

    /// Convenience method for presenting the rendered surface.
    ///
    /// Does nothing if this renderer is not directly rendering to a surface.
    pub fn present(&self) -> Result<(), wgpu::SurfaceError> {
        if let Some((surface, _config)) = &self.surface {
            surface.get_current_texture()?.present();
        }

        Ok(())
    }

    /// Convenience method for clearing the target texture or surface.
    pub fn clear(&self) -> Result<(), wgpu::SurfaceError> {
        let resources = self.resources.lock().unwrap();
        let mut encoder =
            resources
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("WGPURenderer::clear"),
                });

        match (&self.surface, &*self.target.lock().unwrap()) {
            (Some((surface, _)), (None, _)) => {
                encoder.clear_texture(
                    &surface.get_current_texture()?.texture,
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

        resources.queue.submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    pub fn device(&self) -> wgpu::Device {
        self.resources.lock().unwrap().device.clone()
    }

    pub fn target_texture(&self) -> Option<wgpu::Texture> {
        if let (Some(targ), _) = &*self.target.lock().unwrap() {
            return Some(targ.texture().clone());
        }

        None
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
        if self.render_targets.is_none() {
            panic!("Buffer is not yet set up.");
        }

        let encoder = self
            .resources
            .lock()
            .unwrap()
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Inox2DWGPU"),
            });

        let surface_texture = self.surface.as_ref().map(|(surface, config)| {
            (
                surface.get_current_texture(),
                UVec2::new(config.width, config.height),
            )
        });
        let surface_texture = match surface_texture {
            Some((Ok(surface_texture), viewport)) => Some((surface_texture, viewport)),
            Some((Err(e), _)) => return Err(e)?,
            None => None,
        };

        let (view, viewport) = if let Some((surface_texture, viewport)) = &surface_texture {
            (
                surface_texture
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default()),
                *viewport,
            )
        } else if let (Some(device_texture), viewport) = &*self.target.lock().unwrap() {
            (device_texture.view().clone(), *viewport)
        } else {
            return Err("Please resize the renderer before drawing.".into());
        };

        //TODO: read & translate OpenGLRenderer's `on_begin_draw` / `on_end_draw`

        let node_names = puppet
            .nodes()
            .iter()
            .map(|n| (n.uuid, n.name.clone()))
            .collect::<HashMap<_, _>>();
        let viewmatrix = self.camera.matrix(viewport.as_vec2());

        let device = self.resources.lock().unwrap().device.clone();

        Ok(WgpuDrawSession {
            render: self,
            device,
            encoder,
            view,
            viewmatrix,
            node_names,
            last_mask_threshold: 0.0,
            is_in_mask: false,
            is_in_composite: false,
            stencil_reference_value: 1,
        })
    }
}

pub struct WgpuDrawSession<'a, 'window> {
    /// The renderer that owns this draw session.
    render: &'a mut WgpuRenderer<'window>,

    /// Local clone of the device (to avoid overlapping borrows.)
    device: wgpu::Device,

    /// The drawing session's command encoder.
    encoder: wgpu::CommandEncoder,

    /// The output texture to render.
    view: wgpu::TextureView,

    /// The position of the root of our model.
    viewmatrix: Mat4,

    /// All of the node names (for debugging purposes).
    node_names: HashMap<InoxNodeUuid, String>,

    last_mask_threshold: f32,
    is_in_mask: bool,
    is_in_composite: bool,
    stencil_reference_value: u32,
}

impl<'a, 'window> WgpuDrawSession<'a, 'window> {
    fn blend_mode_to_state(state: components::BlendMode) -> wgpu::BlendState {
        let component = match state {
            components::BlendMode::Normal => wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            components::BlendMode::Multiply => wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Dst,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            components::BlendMode::ColorDodge => wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Dst,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            components::BlendMode::LinearDodge => wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            components::BlendMode::Screen => wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrc,
                operation: wgpu::BlendOperation::Add,
            },
            components::BlendMode::ClipToLower => wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::DstAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            components::BlendMode::SliceFromLower => wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Subtract,
            },
        };

        wgpu::BlendState {
            color: component,
            alpha: component,
        }
    }
}

impl<'a, 'window> DrawSession<'a> for WgpuDrawSession<'a, 'window> {
    fn on_begin_masks(&mut self, masks: &components::Masks) {
        self.last_mask_threshold = masks.threshold.clamp(0.0, 1.0);
        //TODO: Enable stencilling on the render target.

        if let Some((composite, surface_stencil)) = self.render.render_targets.as_ref() {
            composite.stencil().clear(&mut self.encoder);
            surface_stencil.clear(&mut self.encoder);
        }
    }

    fn on_begin_mask(&mut self, mask: &components::Mask) {
        self.stencil_reference_value = (mask.mode == components::MaskMode::Mask) as u32;
    }

    fn on_begin_masked_content(&mut self) {
        self.is_in_mask = true;
    }

    fn on_end_mask(&mut self) {
        self.is_in_mask = false;
    }

    fn draw_textured_mesh_content(
        &mut self,
        render_mask: bool,
        components: &drawables::TexturedMeshComponents,
        render_ctx: &render::TexturedMeshRenderCtx,
        id: InoxNodeUuid,
    ) {
        if let Some((composite, surface_stencil)) = self.render.render_targets.as_ref() {
            let gbuffer_color = composite.as_color_attachments();
            let surface_color_view = &self.view;
            let surface_color_attach = Some(wgpu::RenderPassColorAttachment {
                view: &surface_color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            });
            let masked_attach = [surface_color_attach.clone()];
            let unmasked_attach = [surface_color_attach, None, None];

            let color_attachments = if self.is_in_composite {
                if render_mask {
                    &[gbuffer_color[0].clone()]
                } else {
                    gbuffer_color.as_slice()
                }
            } else {
                if render_mask {
                    masked_attach.as_slice()
                } else {
                    unmasked_attach.as_slice()
                }
            };
            let stencil_texture = if self.is_in_composite {
                composite.stencil()
            } else {
                surface_stencil
            };

            let depth_stencil_attachment = if render_mask {
                Some(stencil_texture.as_depth_stencil_attachment_rw())
            } else if self.is_in_mask {
                Some(stencil_texture.as_depth_stencil_attachment_ro())
            } else {
                None
            };

            //TODO: Do we even want blending on in Normal mode?
            let blend = Some(Self::blend_mode_to_state(components.drawable.blending.mode));

            let mut render_pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&format!(
                    "WgpuRenderer::draw_textured_mesh_content - {}",
                    self.node_names
                        .get(&id)
                        .map(|s| s.as_str())
                        .unwrap_or("<NODE UNKNOWN>")
                )),
                color_attachments,
                depth_stencil_attachment,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            let (albedo, bumpmap, emissive) = self.render.textures_for_part(components.texture);
            let (albedo, bumpmap, emissive) = (albedo.clone(), bumpmap.clone(), emissive.clone());

            //TODO: set blend mode
            let uni_in_vert = basic_vert::Input {
                mvp: (self.viewmatrix * *components.transform).to_cols_array_2d(),
                offset: [0.0; 2],
            }
            .into_buffer(&self.render.resources.lock().unwrap().device);

            render_pass.set_vertex_buffer(
                basic_vert::INPUT_INDEX_VERTS,
                self.render.uploads.verts.slice(..),
            );
            render_pass.set_vertex_buffer(
                basic_vert::INPUT_INDEX_UVS,
                self.render.uploads.uvs.slice(..),
            );
            render_pass.set_vertex_buffer(
                basic_vert::INPUT_INDEX_DEFORM,
                self.render.uploads.deforms.slice(..),
            );
            render_pass.set_index_buffer(
                self.render.uploads.indices.slice(..),
                wgpu::IndexFormat::Uint32,
            );

            let mut resources = self.render.resources.lock().unwrap();
            if render_mask {
                let mask_depthstencil = resources.mask_depthstencil.clone();
                //TODO: What happens if a mask is also masked?
                let uni_in_frag = basic_mask_frag::Input {
                    threshold: self.last_mask_threshold,
                }
                .into_buffer(&self.device);
                let frag_binding = resources.part_shader_mask_frag.bind(
                    &self.device,
                    albedo.view(),
                    &self.render.uploads.model_sampler,
                    &uni_in_frag,
                );
                let vert_binding = resources.part_shader_vert.bind(&self.device, &uni_in_vert);
                let pipeline = resources.part_mask_pipeline.with_configuration(
                    &self.device,
                    [color_attachments[0]
                        .as_ref()
                        .map(|ca| ca.view.texture().format())],
                    [blend],
                    [wgpu::ColorWrites::empty()],
                    Some(mask_depthstencil),
                );
                render_pass.set_pipeline(pipeline.pipeline());
                pipeline.bind_frag(&mut render_pass, Some(&frag_binding));
                pipeline.bind_vertex(&mut render_pass, Some(&vert_binding));

                render_pass.set_stencil_reference(self.stencil_reference_value);
            } else {
                let masked_depthstencil = resources.masked_depthstencil.clone();
                let all = wgpu::ColorWrites::ALL;
                //Regular parts
                let formats = [
                    color_attachments[0]
                        .as_ref()
                        .map(|ca| ca.view.texture().format()),
                    color_attachments[1]
                        .as_ref()
                        .map(|ca| ca.view.texture().format()),
                    color_attachments[2]
                        .as_ref()
                        .map(|ca| ca.view.texture().format()),
                ];

                let uni_in_frag = basic_frag::Input {
                    opacity: components.drawable.blending.opacity,
                    multColor: components.drawable.blending.tint.into(),
                    screenColor: components.drawable.blending.screen_tint.into(),
                    emissionStrength: 1.0, //NOTE: OpenGL never sets this.
                }
                .into_buffer(&self.device);
                let frag_binding = resources.part_shader_frag.bind(
                    &self.device,
                    albedo.view(),
                    bumpmap.view(),
                    emissive.view(),
                    &self.render.uploads.model_sampler,
                    &uni_in_frag,
                );
                let vert_binding = resources.part_shader_vert.bind(&self.device, &uni_in_vert);

                let pipeline = if self.is_in_mask {
                    resources.part_pipeline.with_configuration(
                        &self.device,
                        formats,
                        [blend, blend, blend],
                        [all, all, all],
                        Some(masked_depthstencil),
                    )
                } else {
                    resources.part_pipeline.with_configuration(
                        &self.device,
                        formats,
                        [blend, blend, blend],
                        [all, all, all],
                        None,
                    )
                };

                render_pass.set_pipeline(pipeline.pipeline());
                pipeline.bind_frag(&mut render_pass, Some(&frag_binding));
                pipeline.bind_vertex(&mut render_pass, Some(&vert_binding));

                render_pass.set_stencil_reference(1);
                render_pass.set_pipeline(pipeline.pipeline());
            }

            render_pass.draw_indexed(
                render_ctx.index_offset as u32
                    ..(render_ctx.index_offset + render_ctx.index_len as u32),
                0,
                0..1,
            );
        }
    }

    fn begin_composite_content(
        &mut self,
        _as_mask: bool,
        _components: &drawables::CompositeComponents,
        _render_ctx: &render::CompositeRenderCtx,
        _id: InoxNodeUuid,
    ) {
        self.is_in_composite = true;

        if let Some((composite, _surface_stencil)) = self.render.render_targets.as_ref() {
            composite.clear(&mut self.encoder);
        }
    }

    fn finish_composite_content(
        &mut self,
        render_mask: bool,
        components: &drawables::CompositeComponents,
        _render_ctx: &render::CompositeRenderCtx,
        id: InoxNodeUuid,
    ) {
        assert!(self.is_in_composite);
        self.is_in_composite = false;

        if let Some((composite, surface_stencil)) = self.render.render_targets.as_ref() {
            let surface_color_view = &self.view;
            let depth_stencil_attachment = if render_mask {
                Some(surface_stencil.as_depth_stencil_attachment_rw())
            } else if self.is_in_mask {
                Some(surface_stencil.as_depth_stencil_attachment_ro())
            } else {
                None
            };

            //TODO: Do we even want blending on in Normal mode?
            let blend = Some(Self::blend_mode_to_state(components.drawable.blending.mode));

            let color_attachments = [
                Some(wgpu::RenderPassColorAttachment {
                    view: &surface_color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                }),
                None,
                None,
            ];
            let mut render_pass = self.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&format!(
                    "WgpuRenderer::finish_composite_content - {}",
                    self.node_names
                        .get(&id)
                        .map(|s| s.as_str())
                        .unwrap_or("<NODE UNKNOWN>")
                )),
                color_attachments: &color_attachments,
                depth_stencil_attachment,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            render_pass.set_vertex_buffer(
                basic_vert::INPUT_INDEX_VERTS,
                self.render.uploads.verts.slice(..),
            );
            render_pass.set_vertex_buffer(
                basic_vert::INPUT_INDEX_UVS,
                self.render.uploads.uvs.slice(..),
            );
            render_pass.set_vertex_buffer(
                basic_vert::INPUT_INDEX_DEFORM,
                self.render.uploads.deforms.slice(..),
            );
            render_pass.set_index_buffer(
                self.render.uploads.indices.slice(..),
                wgpu::IndexFormat::Uint32,
            );

            let mut resources = self.render.resources.lock().unwrap();
            if render_mask {
                // LOL, the OpenGL renderer didn't handle the "mask by composite" case.
                // I may want to see what Inochi2D's D library does.
                todo!();
            } else {
                let all = wgpu::ColorWrites::ALL;
                let depth_stencil = if self.is_in_mask {
                    Some(resources.masked_depthstencil.clone())
                } else {
                    None
                };
                let formats = [
                    color_attachments[0]
                        .as_ref()
                        .map(|ca| ca.view.texture().format()),
                    None,
                    None,
                ];

                let uni_in_frag = composite_frag::Input {
                    opacity: components.drawable.blending.opacity.clamp(0.0, 1.0),
                    multColor: components
                        .drawable
                        .blending
                        .tint
                        .clamp(glam::Vec3::ZERO, glam::Vec3::ONE)
                        .into(),
                    screenColor: components
                        .drawable
                        .blending
                        .screen_tint
                        .clamp(glam::Vec3::ZERO, glam::Vec3::ONE)
                        .into(),
                }
                .into_buffer(&self.device);
                let frag_binding = resources.composite_shader_frag.bind(
                    &self.device,
                    composite.albedo().view(),
                    composite.emissive().view(),
                    composite.bump().view(),
                    &self.render.uploads.model_sampler,
                    &uni_in_frag,
                );
                let vert_binding = resources.composite_shader_vert.bind(&self.device);

                let pipeline = resources.composite_pipeline.with_configuration(
                    &self.device,
                    formats,
                    [blend, blend, blend],
                    [all, all, all],
                    depth_stencil,
                );

                render_pass.set_pipeline(pipeline.pipeline());
                pipeline.bind_frag(&mut render_pass, Some(&frag_binding));
                pipeline.bind_vertex(&mut render_pass, Some(&vert_binding));
                render_pass.draw_indexed(0..6, 0, 0..1); //TODO: Where do these vertices come from!?!?
            }
        }
    }

    fn on_end_draw(self, _puppet: &inox2d::puppet::Puppet) {
        let end = self.encoder.finish();
        self.render
            .resources
            .lock()
            .unwrap()
            .queue
            .submit(std::iter::once(end));
    }
}
