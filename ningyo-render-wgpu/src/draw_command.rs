use std::num::NonZero;

use inox2d::node::InoxNodeUuid;
use inox2d::node::components;
use inox2d::node::drawables::DrawableKind;
use inox2d::render;

use crate::draw_session::WgpuDrawSession;
use crate::shader::UniformBlock;
use crate::shaders::basic::basic_frag::Viewport;
use crate::shaders::basic::basic_vert;
use crate::texture::DeviceTexture;
use crate::texture::TextureViewExt;
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

        let color = &draw_session.color;
        let composite = &draw_session.composite;
        let stencil = &draw_session.stencil;
        let viewports_config = &draw_session.viewports_config;

        let masked_depthstencil = draw_session.resources.masked_depthstencil.clone();
        let mask_depthstencil = draw_session.resources.mask_depthstencil.clone();
        let ignore_depthstencil = draw_session.resources.ignore_depthstencil.clone();

        // Issue an entirely separate set of drawing commands for each layer in
        // the target texture. We have to do this because wgpu's shader
        // translation is breaking our access to the View Index parameter.
        // The good news is, we'd have to do this anyway for atlassing, soooo
        for current_layer in 0..color.depth_or_array_layers() {
            let multiview_mask = None;
            let mut render_pass: Option<wgpu::RenderPass<'_>> = None;

            let color_view = color.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: current_layer,
                array_layer_count: Some(1),
                ..Default::default()
            });
            let stencil_view = stencil.layer_view(current_layer);
            let composite_albedo_view = composite.albedo().layer_view(current_layer);
            let composite_emissive_view = composite.emissive().layer_view(current_layer);
            let composite_bump_view = composite.bump().layer_view(current_layer);
            let composite_stencil_view = composite.stencil().layer_view(current_layer);

            for command in me.commands.iter() {
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
                                &composite.as_color_attachments(), //NOTE: this does not actually write to these
                                multiview_mask,
                            );
                        }
                    }
                    DrawCommand::ClearCurrentStencil => {
                        // !is_in_composite
                        if render_pass.is_none() {
                            render_pass = None;
                            stencil.clear(&mut draw_session.encoder);
                        } else {
                            let render_pass = render_pass.as_mut().unwrap();
                            stencil.clear_with_render_pass(
                                &draw_session.device,
                                render_pass,
                                &mut draw_session.resources,
                                &[Some(color_view.as_color_attachment()), None, None],
                                multiview_mask,
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
                        let drawable = DrawableKind::new(*id, comps, false).unwrap();
                        let components = match &drawable {
                            DrawableKind::Composite(_) => unreachable!(),
                            DrawableKind::TexturedMesh(components) => components,
                        };

                        let surface_color_attach = Some(color_view.as_color_attachment());
                        let gbuffer_color = [
                            Some(composite_albedo_view.as_color_attachment()),
                            Some(composite_emissive_view.as_color_attachment()),
                            Some(composite_bump_view.as_color_attachment()),
                        ];
                        let unmasked_attach = [surface_color_attach, None, None];
                        let color_attachments = if is_in_composite {
                            gbuffer_color.as_slice()
                        } else {
                            unmasked_attach.as_slice()
                        };

                        if render_pass.is_none() {
                            let depth_stencil_attachment = if is_in_composite {
                                Some(composite_stencil_view.as_depth_stencil_attachment())
                            } else {
                                Some(stencil_view.as_depth_stencil_attachment())
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
                                    multiview_mask,
                                },
                            ));
                        }

                        let render_pass = render_pass.as_mut().unwrap();

                        let blend =
                            Some(Self::blend_mode_to_state(components.drawable.blending.mode));

                        let (albedo, bumpmap, emissive) =
                            Self::textures_for_part(draw_session.uploads, components.texture);
                        let (albedo, bumpmap, emissive) =
                            (albedo.clone(), bumpmap.clone(), emissive.clone());

                        let index = draw_session.buffer_indices.get(&(*id).into()).unwrap();
                        let vert_binding = draw_session.binding_cache.bind_basic_vert(
                            &*draw_session.resources,
                            draw_session.basic_vert_buffer.as_ref().unwrap(),
                            viewports_config,
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

                        if *render_mask {
                            //TODO: What happens if a mask is also masked?
                            let frag_binding = draw_session.binding_cache.bind_basic_mask_frag(
                                &draw_session.resources,
                                albedo.view(),
                                draw_session.basic_mask_frag_buffer.as_ref().unwrap(),
                                viewports_config,
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
                                    multiview_mask,
                                );
                            render_pass.set_pipeline(pipeline.pipeline());
                            pipeline.bind_frag(
                                render_pass,
                                Some(&frag_binding),
                                &[
                                    index.basic_mask_frag.unwrap() as u32,
                                    current_layer * Viewport::static_size() as u32,
                                ],
                            );
                            pipeline.bind_vertex(
                                render_pass,
                                Some(&vert_binding),
                                &[
                                    index.basic_vert.unwrap() as u32,
                                    current_layer * Viewport::static_size() as u32,
                                ],
                            );

                            render_pass.set_stencil_reference(*stencil_reference);
                        } else {
                            //Regular parts
                            let all = wgpu::ColorWrites::ALL;
                            let frag_binding = draw_session.binding_cache.bind_basic_frag(
                                &draw_session.resources,
                                albedo.view(),
                                emissive.view(),
                                bumpmap.view(),
                                draw_session.basic_frag_buffer.as_ref().unwrap(),
                                viewports_config,
                            );

                            let pipeline = if *using_mask {
                                draw_session.resources.part_pipeline_with_configuration(
                                    &draw_session.device,
                                    formats,
                                    [blend, blend, blend],
                                    [all, all, all],
                                    Some(masked_depthstencil.clone()),
                                    multiview_mask,
                                )
                            } else {
                                draw_session.resources.part_pipeline_with_configuration(
                                    &draw_session.device,
                                    formats,
                                    [blend, blend, blend],
                                    [all, all, all],
                                    Some(ignore_depthstencil.clone()),
                                    multiview_mask,
                                )
                            };

                            render_pass.set_pipeline(pipeline.pipeline());
                            pipeline.bind_frag(
                                render_pass,
                                Some(&frag_binding),
                                &[
                                    index.basic_frag.unwrap() as u32,
                                    current_layer * Viewport::static_size() as u32,
                                ],
                            );
                            pipeline.bind_vertex(
                                render_pass,
                                Some(&vert_binding),
                                &[
                                    index.basic_vert.unwrap() as u32,
                                    current_layer * Viewport::static_size() as u32,
                                ],
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
                        let drawable = DrawableKind::new(*id, comps, false).unwrap();
                        let components = match &drawable {
                            DrawableKind::Composite(components) => components,
                            DrawableKind::TexturedMesh(_) => unreachable!(),
                        };

                        assert!(is_in_composite);
                        is_in_composite = false;

                        let depth_stencil_attachment =
                            Some(stencil_view.as_depth_stencil_attachment());

                        //TODO: Do we even want blending on in Normal mode?
                        let blend =
                            Some(Self::blend_mode_to_state(components.drawable.blending.mode));

                        let color_attachments =
                            [Some(color_view.as_color_attachment()), None, None];

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
                                    multiview_mask,
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

                        if *render_mask {
                            // LOL, the OpenGL renderer didn't handle the "mask by composite" case.
                            // I may want to see what Inochi2D's D library does.
                            todo!();
                        } else {
                            let all = wgpu::ColorWrites::ALL;
                            let depth_stencil = if *using_mask {
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

                            let index = draw_session.buffer_indices.get(&(*id).into()).unwrap();
                            let vert_binding = draw_session
                                .binding_cache
                                .bind_composite_vert(&draw_session.resources, viewports_config);
                            let frag_binding = draw_session.binding_cache.bind_composite_frag(
                                &draw_session.resources,
                                &composite_albedo_view,
                                &composite_emissive_view,
                                &composite_bump_view,
                                draw_session.composite_frag_buffer.as_ref().unwrap(),
                                viewports_config,
                            );

                            let pipeline = draw_session
                                .resources
                                .composite_pipeline_with_configuration(
                                    &draw_session.device,
                                    formats,
                                    [blend, blend, blend],
                                    [all, all, all],
                                    depth_stencil,
                                    multiview_mask,
                                );

                            render_pass.set_pipeline(pipeline.pipeline());
                            pipeline.bind_frag(
                                &mut render_pass,
                                Some(&frag_binding),
                                &[
                                    index.composite_frag.unwrap() as u32,
                                    current_layer * Viewport::static_size() as u32,
                                ],
                            );
                            pipeline.bind_vertex(
                                &mut render_pass,
                                Some(&vert_binding),
                                &[current_layer * Viewport::static_size() as u32],
                            );
                            render_pass.draw_indexed(0..6, 0, 0..1); //TODO: Where do these vertices come from!?!?
                        }
                    }
                }
            }
        }

        me.commands.drain(..);
    }
}
