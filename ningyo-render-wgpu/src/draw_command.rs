use inox2d::node::InoxNodeUuid;
use inox2d::node::components;
use inox2d::node::drawables::DrawableKind;
use inox2d::render;

use crate::draw_session::WgpuDrawSession;
use crate::shaders::basic::basic_vert;
use crate::texture::DeviceTexture;
use crate::uploads::WgpuUploads;

pub enum DrawCommand {
    ClearCurrentStencil,
    DrawPart {
        render_mask: bool,
        using_mask: bool,
        stencil_reference: u32,
        id: InoxNodeUuid,
        render_ctx: render::TexturedMeshRenderCtx,
    },
    BeginComposite,
    EndComposite {
        render_mask: bool,
        using_mask: bool,
        id: InoxNodeUuid,
    },
}

#[derive(Default)]
pub struct DrawCommandList {
    commands: Vec<DrawCommand>,
}

impl DrawCommandList {
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

    pub fn clear_current_stencil(&mut self) {
        self.commands.push(DrawCommand::ClearCurrentStencil);
    }

    pub fn draw_part(
        &mut self,
        render_mask: bool,
        using_mask: bool,
        stencil_reference: u32,
        id: InoxNodeUuid,
        render_ctx: &render::TexturedMeshRenderCtx,
    ) {
        self.commands.push(DrawCommand::DrawPart {
            render_mask,
            using_mask,
            stencil_reference,
            id,
            render_ctx: render::TexturedMeshRenderCtx {
                index_offset: render_ctx.index_offset,
                index_len: render_ctx.index_len,
                vert_offset: render_ctx.vert_offset,
                vert_len: render_ctx.vert_len,
            },
        });
    }

    pub fn begin_composite(&mut self) {
        self.commands.push(DrawCommand::BeginComposite);
    }

    pub fn end_composite(&mut self, render_mask: bool, using_mask: bool, id: InoxNodeUuid) {
        self.commands.push(DrawCommand::EndComposite {
            render_mask,
            using_mask,
            id,
        });
    }

    pub fn textures_for_part<'a>(
        uploads: &'a WgpuUploads,
        part: &components::TexturedMesh,
    ) -> (&'a DeviceTexture, &'a DeviceTexture, &'a DeviceTexture) {
        (
            &uploads.model_textures[part.tex_albedo.raw()],
            &uploads.model_textures[part.tex_bumpmap.raw()],
            &uploads.model_textures[part.tex_emissive.raw()],
        )
    }

    pub fn flush(draw_session: &mut WgpuDrawSession<'_>, puppet: &inox2d::puppet::Puppet) {
        let mut is_in_composite = false;
        let me = &mut draw_session.draw_commands;

        let (composite, surface_stencil) = draw_session.render_targets.as_ref().unwrap();
        let surface_color_view = &draw_session.view;

        let masked_depthstencil = draw_session.resources.masked_depthstencil.clone();
        let mask_depthstencil = draw_session.resources.mask_depthstencil.clone();
        let ignore_depthstencil = draw_session.resources.ignore_depthstencil.clone();

        let mut render_pass: Option<wgpu::RenderPass<'_>> = None;

        for command in me.commands.drain(..) {
            match command {
                DrawCommand::ClearCurrentStencil if is_in_composite => {
                    if render_pass.is_none() {
                        render_pass = None; //Borrowck can't tell otherwise
                        composite.stencil().clear(&mut draw_session.encoder);
                    } else {
                        let render_pass = render_pass.as_mut().unwrap();
                        composite.stencil().clear_with_render_pass(
                            &draw_session.device,
                            render_pass,
                            &mut draw_session.resources,
                            &composite.as_color_attachments(),
                        );
                    }
                }
                DrawCommand::ClearCurrentStencil => {
                    // !is_in_composite
                    if render_pass.is_none() {
                        render_pass = None;
                        surface_stencil.clear(&mut draw_session.encoder);
                    } else {
                        let render_pass = render_pass.as_mut().unwrap();
                        surface_stencil.clear_with_render_pass(
                            &draw_session.device,
                            render_pass,
                            &mut draw_session.resources,
                            &[
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
                            ],
                        );
                    }
                }
                DrawCommand::DrawPart {
                    render_mask,
                    using_mask,
                    stencil_reference,
                    id,
                    render_ctx,
                } => {
                    let comps = puppet.world();
                    let drawable = DrawableKind::new(id, comps, false).unwrap();
                    let components = match &drawable {
                        DrawableKind::Composite(_) => unreachable!(),
                        DrawableKind::TexturedMesh(components) => components,
                    };

                    let surface_color_attach = Some(wgpu::RenderPassColorAttachment {
                        view: &surface_color_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    });
                    let gbuffer_color = composite.as_color_attachments();
                    let unmasked_attach = [surface_color_attach, None, None];
                    let color_attachments = if is_in_composite {
                        gbuffer_color.as_slice()
                    } else {
                        unmasked_attach.as_slice()
                    };

                    if render_pass.is_none() {
                        let depth_stencil_attachment = if is_in_composite {
                            Some(composite.stencil().as_depth_stencil_attachment_rw())
                        } else {
                            Some(surface_stencil.as_depth_stencil_attachment_rw())
                        };

                        drop(render_pass);

                        render_pass = Some(draw_session.encoder.begin_render_pass(
                            &wgpu::RenderPassDescriptor {
                                label: Some(&format!(
                                        "WgpuRenderer::draw_textured_mesh_content - {}",
                                        draw_session
                                            .node_names
                                            .get(&id)
                                            .map(|s| s.as_str())
                                            .unwrap_or("<NODE UNKNOWN>")
                                    )),
                                color_attachments,
                                depth_stencil_attachment,
                                occlusion_query_set: None,
                                timestamp_writes: None,
                                multiview_mask: None,
                            },
                        ));
                    }

                    let render_pass = render_pass.as_mut().unwrap();

                    let blend = Some(Self::blend_mode_to_state(components.drawable.blending.mode));

                    let (albedo, bumpmap, emissive) =
                        Self::textures_for_part(draw_session.uploads, components.texture);
                    let (albedo, bumpmap, emissive) =
                        (albedo.clone(), bumpmap.clone(), emissive.clone());

                    let index = draw_session.buffer_indices.get(&id.into()).unwrap();
                    let vert_binding = draw_session.binding_cache.bind_basic_vert(
                        &*draw_session.resources,
                        draw_session.basic_vert_buffer.as_ref().unwrap(),
                        draw_session.viewports_config,
                    );

                    render_pass.set_vertex_buffer(
                        basic_vert::INPUT_INDEX_VERTS,
                        draw_session.uploads.verts.slice(..),
                    );
                    render_pass.set_vertex_buffer(
                        basic_vert::INPUT_INDEX_UVS,
                        draw_session.uploads.uvs.slice(..),
                    );
                    render_pass.set_vertex_buffer(
                        basic_vert::INPUT_INDEX_DEFORM,
                        draw_session.uploads.deforms.slice(..),
                    );
                    render_pass.set_index_buffer(
                        draw_session.uploads.indices.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );

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

                    if render_mask {
                        //TODO: What happens if a mask is also masked?
                        let frag_binding = draw_session.binding_cache.bind_basic_mask_frag(
                            &draw_session.resources,
                            albedo.view(),
                            draw_session.basic_mask_frag_buffer.as_ref().unwrap(),
                            draw_session.viewports_config,
                        );
                        let pipeline = draw_session
                            .resources
                            .part_mask_pipeline_with_configuration(
                                &draw_session.device,
                                formats,
                                [blend, blend, blend],
                                [
                                    wgpu::ColorWrites::empty(),
                                    wgpu::ColorWrites::empty(),
                                    wgpu::ColorWrites::empty(),
                                ],
                                Some(mask_depthstencil.clone()),
                            );
                        render_pass.set_pipeline(pipeline.pipeline());
                        pipeline.bind_frag(
                            render_pass,
                            Some(&frag_binding),
                            &[index.basic_mask_frag.unwrap() as u32],
                        );
                        pipeline.bind_vertex(
                            render_pass,
                            Some(&vert_binding),
                            &[index.basic_vert.unwrap() as u32],
                        );

                        render_pass.set_stencil_reference(stencil_reference);
                    } else {
                        //Regular parts
                        let all = wgpu::ColorWrites::ALL;
                        let frag_binding = draw_session.binding_cache.bind_basic_frag(
                            &draw_session.resources,
                            albedo.view(),
                            emissive.view(),
                            bumpmap.view(),
                            draw_session.basic_frag_buffer.as_ref().unwrap(),
                            draw_session.viewports_config,
                        );

                        let pipeline = if using_mask {
                            draw_session.resources.part_pipeline_with_configuration(
                                &draw_session.device,
                                formats,
                                [blend, blend, blend],
                                [all, all, all],
                                Some(masked_depthstencil.clone()),
                            )
                        } else {
                            draw_session.resources.part_pipeline_with_configuration(
                                &draw_session.device,
                                formats,
                                [blend, blend, blend],
                                [all, all, all],
                                Some(ignore_depthstencil.clone()),
                            )
                        };

                        render_pass.set_pipeline(pipeline.pipeline());
                        pipeline.bind_frag(
                            render_pass,
                            Some(&frag_binding),
                            &[index.basic_frag.unwrap() as u32],
                        );
                        pipeline.bind_vertex(
                            render_pass,
                            Some(&vert_binding),
                            &[index.basic_vert.unwrap() as u32],
                        );

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
                DrawCommand::BeginComposite => {
                    render_pass = None;
                    composite.clear(&mut draw_session.encoder);
                    is_in_composite = true;
                }
                DrawCommand::EndComposite {
                    render_mask,
                    using_mask,
                    id,
                } => {
                    let comps = puppet.world();
                    let drawable = DrawableKind::new(id, comps, false).unwrap();
                    let components = match &drawable {
                        DrawableKind::Composite(components) => components,
                        DrawableKind::TexturedMesh(_) => unreachable!(),
                    };

                    assert!(is_in_composite);
                    is_in_composite = false;

                    let surface_color_view = &draw_session.view;
                    let depth_stencil_attachment =
                        Some(surface_stencil.as_depth_stencil_attachment_rw());

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

                    // Strictly speaking, we will never be able to batch the end of a
                    // composite operation. (Or so I think?!)
                    render_pass = None;
                    let mut render_pass =
                        draw_session
                            .encoder
                            .begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some(&format!(
                                    "WgpuRenderer::finish_composite_content - {}",
                                    draw_session
                                        .node_names
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
                        draw_session.uploads.verts.slice(..),
                    );
                    render_pass.set_vertex_buffer(
                        basic_vert::INPUT_INDEX_UVS,
                        draw_session.uploads.uvs.slice(..),
                    );
                    render_pass.set_vertex_buffer(
                        basic_vert::INPUT_INDEX_DEFORM,
                        draw_session.uploads.deforms.slice(..),
                    );
                    render_pass.set_index_buffer(
                        draw_session.uploads.indices.slice(..),
                        wgpu::IndexFormat::Uint32,
                    );

                    if render_mask {
                        // LOL, the OpenGL renderer didn't handle the "mask by composite" case.
                        // I may want to see what Inochi2D's D library does.
                        todo!();
                    } else {
                        let all = wgpu::ColorWrites::ALL;
                        let depth_stencil = if using_mask {
                            Some(masked_depthstencil.clone())
                        } else {
                            Some(ignore_depthstencil.clone())
                        };
                        let formats = [
                            color_attachments[0]
                                .as_ref()
                                .map(|ca| ca.view.texture().format()),
                            None,
                            None,
                        ];

                        let index = draw_session.buffer_indices.get(&id.into()).unwrap();
                        let vert_binding = draw_session.binding_cache.bind_composite_vert(
                            &draw_session.resources,
                            draw_session.viewports_config,
                        );
                        let frag_binding = draw_session.binding_cache.bind_composite_frag(
                            &draw_session.resources,
                            composite.albedo().view(),
                            composite.emissive().view(),
                            composite.bump().view(),
                            draw_session.composite_frag_buffer.as_ref().unwrap(),
                            draw_session.viewports_config,
                        );

                        let pipeline = draw_session
                            .resources
                            .composite_pipeline_with_configuration(
                                &draw_session.device,
                                formats,
                                [blend, blend, blend],
                                [all, all, all],
                                depth_stencil,
                            );

                        render_pass.set_pipeline(pipeline.pipeline());
                        pipeline.bind_frag(
                            &mut render_pass,
                            Some(&frag_binding),
                            &[index.composite_frag.unwrap() as u32],
                        );
                        pipeline.bind_vertex(&mut render_pass, Some(&vert_binding), &[]);
                        render_pass.draw_indexed(0..6, 0, 0..1); //TODO: Where do these vertices come from!?!?
                    }
                }
            }
        }
    }
}
