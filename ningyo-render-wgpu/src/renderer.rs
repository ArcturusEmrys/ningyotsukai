use glam::Mat4;
use glam::UVec2;
use inox2d::math::camera::Camera;
use inox2d::model::Model;
use inox2d::node::drawables::DrawableKind;
use inox2d::node::{InoxNodeUuid, components, drawables};
use inox2d::render::CompositeRenderCtx;
//hey wait a second that's just a u32 newtype! UUIDs are four of those!
use inox2d::render::{self, DrawSession, InoxRenderer};
use ningyo_extensions::CurrentSurfaceTextureExt;
use std::collections::BTreeMap;
use std::error::Error;
use std::num::NonZero;
use std::sync::{Arc, Mutex};
use wgpu;

use crate::buffer_builder::BufferBuilder;
use crate::shader::UniformBlock;
use crate::shaders::basic::{basic_frag, basic_mask_frag, basic_vert, composite_frag};
use crate::texture::{DepthStencilTexture, DeviceTexture, GBuffer};

use std::collections::HashMap;

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

    /// Builder for basic part vertex uniforms
    builder_basic_vert: BufferBuilder<basic_vert::Input>,

    /// Builder for basic part frag uniforms
    builder_basic_frag: BufferBuilder<basic_frag::Input>,

    /// Builder for basic mask frag uniforms
    builder_basic_mask_frag: BufferBuilder<basic_mask_frag::Input>,

    /// Builder for composite frag uniforms
    builder_composite_frag: BufferBuilder<composite_frag::Input>,

    /// An index into the buffers for each node.
    buffer_indices: BTreeMap<u32, BufferIndices>,

    uploads: WgpuUploads,
    resources: Arc<Mutex<WgpuResources>>,
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
        let uploads = WgpuUploads::new(model, &mut *resources)?;
        drop(resources);

        Ok(WgpuRenderer {
            surface: None,

            // The 640x480 size is a placeholder, we're waiting for a resize.
            target: Arc::new(Mutex::new((None, UVec2::new(640, 480)))),
            camera: Camera::default(),
            render_targets: None,
            uploads,
            resources: resources_arc,
            builder_basic_frag: BufferBuilder::new(wgpu::Limits::default()),
            builder_basic_mask_frag: BufferBuilder::new(wgpu::Limits::default()),
            builder_basic_vert: BufferBuilder::new(wgpu::Limits::default()),
            builder_composite_frag: BufferBuilder::new(wgpu::Limits::default()),
            buffer_indices: Default::default(),
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
            let resources = self.resources.lock().unwrap();
            let mut encoder =
                resources
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Inox2D texture resizes"),
                    });

            if let Some((surface, config)) = &mut self.surface {
                config.width = width;
                config.height = height;
                surface.configure(&resources.device, config);
            } else if self.target.lock().unwrap().0.is_none() {
                panic!("Render target texture must have been set before resize!!!")
            }

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

        {
            let mut target = self.target.lock().unwrap();
            target.1 = UVec2::new(new_target.texture().width(), new_target.texture().height());
            target.0 = Some(new_target);
        }

        self.resize(width, height)
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
                surface.get_current_texture().as_surface_texture(),
                UVec2::new(config.width, config.height),
            )
        });
        let surface_texture = match surface_texture {
            Some((
                Ok(ningyo_extensions::SurfaceTexture {
                    texture,
                    optimal: _optimal,
                }),
                viewport,
            )) => Some((texture, viewport)),
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

        let mut session = WgpuDrawSession {
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
            basic_vert_buffer: None,
            basic_frag_buffer: None,
            basic_mask_frag_buffer: None,
            composite_frag_buffer: None,
        };

        session.buffer_prepass(puppet);

        Ok(session)
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

    /// The currently active set of vert shader uniforms.
    basic_vert_buffer: Option<wgpu::Buffer>,

    /// The currently active set of frag shader uniforms (for non-mask usage)
    basic_frag_buffer: Option<wgpu::Buffer>,

    /// The currently active set of frag shader uniforms (for mask usage)
    basic_mask_frag_buffer: Option<wgpu::Buffer>,

    /// The currently active set of composite deferred pass uniforms
    composite_frag_buffer: Option<wgpu::Buffer>,

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

    /// Fill our uniform buffers with all the data we will need during
    /// rendering.
    ///
    /// This prepass is necessary as individual per-frame buffer uploads can
    /// occupy up to 3ms of render time (tested on Arcturus Emrys himself)
    fn buffer_prepass(&mut self, puppet: &inox2d::puppet::Puppet) {
        for (_, indices) in self.render.buffer_indices.iter_mut() {
            indices.clear();
        }

        for uuid in puppet
            .render_ctx
            .as_ref()
            .expect("RenderCtx of puppet must be initialized before calling draw().")
            .root_drawables_zsorted()
            .iter()
        {
            self.buffer_prepass_drawable(puppet, *uuid, false);
        }

        self.basic_vert_buffer = Some(self.render.builder_basic_vert.commit(&self.device));
        self.basic_frag_buffer = Some(self.render.builder_basic_frag.commit(&self.device));
        self.basic_mask_frag_buffer =
            Some(self.render.builder_basic_mask_frag.commit(&self.device));
        self.composite_frag_buffer = Some(self.render.builder_composite_frag.commit(&self.device));
    }

    fn buffer_prepass_drawable(
        &mut self,
        puppet: &inox2d::puppet::Puppet,
        uuid: InoxNodeUuid,
        render_mask: bool,
    ) {
        let comps = puppet.world();
        let drawable = DrawableKind::new(uuid, comps, false);

        let masks = match &drawable {
            Some(DrawableKind::Composite(components)) => &components.drawable.masks,
            Some(DrawableKind::TexturedMesh(components)) => &components.drawable.masks,
            None => &None,
        };

        if let Some(masks) = masks {
            self.last_mask_threshold = masks.threshold;
            for mask in &masks.masks {
                self.buffer_prepass_drawable(puppet, mask.source, true);
            }
        }

        if matches!(drawable, Some(DrawableKind::Composite(_))) {
            let render_ctx = comps.get::<CompositeRenderCtx>(uuid).unwrap();

            for child_uuid in &render_ctx.zsorted_children_list {
                self.buffer_prepass_drawable(puppet, *child_uuid, false);
            }
        }

        let index = self.render.buffer_indices.entry(uuid.into()).or_default();

        match &drawable {
            Some(DrawableKind::Composite(components)) => {
                if index.composite_frag.is_none() {
                    index.composite_frag = Some(
                        self.render
                            .builder_composite_frag
                            .insert(composite_frag::Input {
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
                            }),
                    );
                }
            }
            Some(DrawableKind::TexturedMesh(components)) => {
                if index.basic_vert.is_none() {
                    index.basic_vert =
                        Some(self.render.builder_basic_vert.insert(basic_vert::Input {
                            mvp: (self.viewmatrix * *components.transform).to_cols_array_2d(),
                            offset: [0.0; 2],
                        }));
                }

                if render_mask {
                    if index.basic_mask_frag.is_none() {
                        index.basic_mask_frag = Some(self.render.builder_basic_mask_frag.insert(
                            basic_mask_frag::Input {
                                threshold: self.last_mask_threshold,
                            },
                        ));
                    }
                } else {
                    if index.basic_frag.is_none() {
                        index.basic_frag =
                            Some(self.render.builder_basic_frag.insert(basic_frag::Input {
                                opacity: components.drawable.blending.opacity,
                                multColor: components.drawable.blending.tint.into(),
                                screenColor: components.drawable.blending.screen_tint.into(),
                                emissionStrength: 1.0, //NOTE: OpenGL never sets this.
                            }));
                    }
                }
            }
            None => {}
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

            let index = self.render.buffer_indices.get(&id.into()).unwrap();

            let uni_in_vert = wgpu::BufferBinding {
                buffer: self.basic_vert_buffer.as_ref().unwrap(),
                offset: index.basic_vert.unwrap() as u64,
                size: Some(
                    NonZero::new(size_of::<<basic_vert::Input as UniformBlock>::Buffer>() as u64)
                        .unwrap(),
                ),
            };

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
                let uni_in_frag = wgpu::BufferBinding {
                    buffer: self.basic_mask_frag_buffer.as_ref().unwrap(),
                    offset: index.basic_mask_frag.unwrap() as u64,
                    size: Some(
                        NonZero::new(
                            size_of::<<basic_mask_frag::Input as UniformBlock>::Buffer>() as u64,
                        )
                        .unwrap(),
                    ),
                };
                let frag_binding = resources.part_shader_mask_frag.bind(
                    &self.device,
                    albedo.view(),
                    &resources.model_sampler,
                    uni_in_frag,
                );
                let vert_binding = resources.part_shader_vert.bind(&self.device, uni_in_vert);
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

                let uni_in_frag = wgpu::BufferBinding {
                    buffer: self.basic_frag_buffer.as_ref().unwrap(),
                    offset: index.basic_frag.unwrap() as u64,
                    size: Some(
                        NonZero::new(
                            size_of::<<basic_frag::Input as UniformBlock>::Buffer>() as u64
                        )
                        .unwrap(),
                    ),
                };
                let frag_binding = resources.part_shader_frag.bind(
                    &self.device,
                    albedo.view(),
                    bumpmap.view(),
                    emissive.view(),
                    &resources.model_sampler,
                    uni_in_frag,
                );
                let vert_binding = resources.part_shader_vert.bind(&self.device, uni_in_vert);

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

                let index = self.render.buffer_indices.get(&id.into()).unwrap();
                let uni_in_frag = wgpu::BufferBinding {
                    buffer: self.composite_frag_buffer.as_ref().unwrap(),
                    offset: index.composite_frag.unwrap() as u64,
                    size: Some(
                        NonZero::new(
                            size_of::<<composite_frag::Input as UniformBlock>::Buffer>() as u64
                        )
                        .unwrap(),
                    ),
                };
                let frag_binding = resources.composite_shader_frag.bind(
                    &self.device,
                    composite.albedo().view(),
                    composite.emissive().view(),
                    composite.bump().view(),
                    &resources.model_sampler,
                    uni_in_frag,
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
