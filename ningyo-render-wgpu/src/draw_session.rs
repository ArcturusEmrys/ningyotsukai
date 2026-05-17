use glam::Mat4;
use glam::UVec2;
use inox2d::node::drawables::DrawableKind;
use inox2d::node::{InoxNodeUuid, components, drawables};
use inox2d::render::CompositeRenderCtx;
//hey wait a second that's just a u32 newtype! UUIDs are four of those!
use inox2d::render::{self, DrawSession};
use ningyo_extensions::CurrentSurfaceTextureExt;
use std::error::Error;
use std::num::NonZero;
use std::sync::MutexGuard;
use wgpu;

use crate::WgpuRenderer;
use crate::buffer_builder::BufferBuilder;
use crate::shader::UniformBlock;
use crate::shaders::basic::{basic_frag, basic_mask_frag, basic_vert, composite_frag};
use crate::texture::{DepthStencilTexture, DeviceTexture, GBuffer};

use std::collections::HashMap;

use crate::renderer::BufferIndices;
use crate::resources::WgpuResources;
use crate::uploads::WgpuUploads;

pub struct WgpuDrawSession<'a> {
    /// The rendering resources for this draw session.
    ///
    /// We keep the resources locked throughout the draw session to avoid
    /// contention between multiple renderers.
    resources: MutexGuard<'a, WgpuResources>,

    /// The uploads for the particular model that we will be drawing.
    uploads: &'a WgpuUploads,

    /// All textures used as render targets, excluding the surface color
    /// buffer.
    ///
    /// GBuffer is used solely for composite rendering, where rendered pixels
    /// are used for a deferred shading pass.
    render_targets: &'a mut Option<(GBuffer, DepthStencilTexture)>,

    buffer_indices: &'a mut HashMap<u32, BufferIndices>,
    builder_basic_vert: &'a mut BufferBuilder<basic_vert::Input>,
    builder_basic_frag: &'a mut BufferBuilder<basic_frag::Input>,
    builder_basic_mask_frag: &'a mut BufferBuilder<basic_mask_frag::Input>,
    builder_composite_frag: &'a mut BufferBuilder<composite_frag::Input>,

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

impl<'a> WgpuDrawSession<'a> {
    pub fn begin(
        renderer: &'a mut WgpuRenderer<'_>,
        puppet: &inox2d::puppet::Puppet,
    ) -> Result<Self, Box<dyn Error>> {
        if renderer.render_targets.is_none() {
            panic!("Buffer is not yet set up.");
        }

        let resources = renderer.resources.lock().unwrap();

        let encoder = resources
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Inox2DWGPU"),
            });

        let surface_texture = renderer.surface.as_ref().map(|(surface, config)| {
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
        } else if let (Some(device_texture), viewport) = &renderer.target {
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
        let viewmatrix = renderer.camera.matrix(viewport.as_vec2());

        let device = resources.device.clone();

        let mut session = WgpuDrawSession {
            resources,
            uploads: &renderer.uploads,
            render_targets: &mut renderer.render_targets,
            buffer_indices: &mut renderer.buffer_indices,
            builder_basic_vert: &mut renderer.builder_basic_vert,
            builder_basic_frag: &mut renderer.builder_basic_frag,
            builder_basic_mask_frag: &mut renderer.builder_basic_mask_frag,
            builder_composite_frag: &mut renderer.builder_composite_frag,
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
        for (_, indices) in self.buffer_indices.iter_mut() {
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

        self.basic_vert_buffer = Some(self.builder_basic_vert.commit(&self.device));
        self.basic_frag_buffer = Some(self.builder_basic_frag.commit(&self.device));
        self.basic_mask_frag_buffer = Some(self.builder_basic_mask_frag.commit(&self.device));
        self.composite_frag_buffer = Some(self.builder_composite_frag.commit(&self.device));
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

        let index = self.buffer_indices.entry(uuid.into()).or_default();

        match &drawable {
            Some(DrawableKind::Composite(components)) => {
                if index.composite_frag.is_none() {
                    index.composite_frag = Some(
                        self.builder_composite_frag.insert(composite_frag::Input {
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
                    index.basic_vert = Some(self.builder_basic_vert.insert(basic_vert::Input {
                        mvp: (self.viewmatrix * *components.transform).to_cols_array_2d(),
                        offset: [0.0; 2],
                    }));
                }

                if render_mask {
                    if index.basic_mask_frag.is_none() {
                        index.basic_mask_frag =
                            Some(self.builder_basic_mask_frag.insert(basic_mask_frag::Input {
                                threshold: self.last_mask_threshold,
                            }));
                    }
                } else {
                    if index.basic_frag.is_none() {
                        index.basic_frag =
                            Some(self.builder_basic_frag.insert(basic_frag::Input {
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

impl<'a> DrawSession<'a> for WgpuDrawSession<'a> {
    fn on_begin_masks(&mut self, masks: &components::Masks) {
        self.last_mask_threshold = masks.threshold.clamp(0.0, 1.0);
        //TODO: Enable stencilling on the render target.

        if let Some((composite, surface_stencil)) = self.render_targets.as_ref() {
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
        if let Some((composite, surface_stencil)) = self.render_targets.as_ref() {
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

            let (albedo, bumpmap, emissive) = self.textures_for_part(components.texture);
            let (albedo, bumpmap, emissive) = (albedo.clone(), bumpmap.clone(), emissive.clone());

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

            let index = self.buffer_indices.get(&id.into()).unwrap();

            let uni_in_vert = wgpu::BufferBinding {
                buffer: self.basic_vert_buffer.as_ref().unwrap(),
                offset: index.basic_vert.unwrap() as u64,
                size: Some(
                    NonZero::new(size_of::<<basic_vert::Input as UniformBlock>::Buffer>() as u64)
                        .unwrap(),
                ),
            };

            render_pass
                .set_vertex_buffer(basic_vert::INPUT_INDEX_VERTS, self.uploads.verts.slice(..));
            render_pass.set_vertex_buffer(basic_vert::INPUT_INDEX_UVS, self.uploads.uvs.slice(..));
            render_pass.set_vertex_buffer(
                basic_vert::INPUT_INDEX_DEFORM,
                self.uploads.deforms.slice(..),
            );
            render_pass.set_index_buffer(self.uploads.indices.slice(..), wgpu::IndexFormat::Uint32);

            if render_mask {
                let mask_depthstencil = self.resources.mask_depthstencil.clone();
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
                let frag_binding = self.resources.part_shader_mask_frag.bind(
                    &self.device,
                    albedo.view(),
                    &self.resources.model_sampler,
                    uni_in_frag,
                );
                let vert_binding = self
                    .resources
                    .part_shader_vert
                    .bind(&self.device, uni_in_vert);
                let pipeline = self.resources.part_mask_pipeline.with_configuration(
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
                let masked_depthstencil = self.resources.masked_depthstencil.clone();
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
                let frag_binding = self.resources.part_shader_frag.bind(
                    &self.device,
                    albedo.view(),
                    bumpmap.view(),
                    emissive.view(),
                    &self.resources.model_sampler,
                    uni_in_frag,
                );
                let vert_binding = self
                    .resources
                    .part_shader_vert
                    .bind(&self.device, uni_in_vert);

                let pipeline = if self.is_in_mask {
                    self.resources.part_pipeline.with_configuration(
                        &self.device,
                        formats,
                        [blend, blend, blend],
                        [all, all, all],
                        Some(masked_depthstencil),
                    )
                } else {
                    self.resources.part_pipeline.with_configuration(
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

        if let Some((composite, _surface_stencil)) = self.render_targets.as_ref() {
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

        if let Some((composite, surface_stencil)) = self.render_targets.as_ref() {
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

            render_pass
                .set_vertex_buffer(basic_vert::INPUT_INDEX_VERTS, self.uploads.verts.slice(..));
            render_pass.set_vertex_buffer(basic_vert::INPUT_INDEX_UVS, self.uploads.uvs.slice(..));
            render_pass.set_vertex_buffer(
                basic_vert::INPUT_INDEX_DEFORM,
                self.uploads.deforms.slice(..),
            );
            render_pass.set_index_buffer(self.uploads.indices.slice(..), wgpu::IndexFormat::Uint32);

            if render_mask {
                // LOL, the OpenGL renderer didn't handle the "mask by composite" case.
                // I may want to see what Inochi2D's D library does.
                todo!();
            } else {
                let all = wgpu::ColorWrites::ALL;
                let depth_stencil = if self.is_in_mask {
                    Some(self.resources.masked_depthstencil.clone())
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

                let index = self.buffer_indices.get(&id.into()).unwrap();
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
                let frag_binding = self.resources.composite_shader_frag.bind(
                    &self.device,
                    composite.albedo().view(),
                    composite.emissive().view(),
                    composite.bump().view(),
                    &self.resources.model_sampler,
                    uni_in_frag,
                );
                let vert_binding = self.resources.composite_shader_vert.bind(&self.device);

                let pipeline = self.resources.composite_pipeline.with_configuration(
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
        self.resources.queue.submit(std::iter::once(end));
    }
}
