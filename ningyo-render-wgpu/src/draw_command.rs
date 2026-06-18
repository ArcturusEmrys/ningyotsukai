use inox2d::node::InoxNodeUuid;
use inox2d::node::drawables::DrawableKind;
use rayon::current_num_threads;

#[cfg(feature = "timing")]
use crate::binding_cache::BindingCache;
use crate::blend::blend_mode_to_state;
use crate::draw_session::WgpuDrawSession;
use crate::shader::UniformBlock;
use crate::shaders::basic::basic_frag::Viewport;
use crate::shaders::basic::basic_vert;
use crate::texture::TextureViewExt;

use rayon::prelude::*;

use crossbeam::channel::{Sender, bounded};
use std::cmp::max;
use std::collections::HashMap;

#[cfg(not(feature = "timing"))]
type FlushResult = (usize, wgpu::CommandBuffer, BindingCache<'static>);

#[cfg(feature = "timing")]
type FlushResult = (
    usize,
    wgpu::CommandBuffer,
    BindingCache<'static>,
    u32,
    std::time::Duration,
);

#[derive(Debug)]
pub enum DrawCommand {
    ClearCurrentStencil {
        to_composite: bool,
    },
    DrawPart {
        render_mask: bool,
        using_mask: bool,
        stencil_reference: u32,
        blend_mode: Option<wgpu::BlendState>,
        indirect_offset: wgpu::BufferAddress,
        indirect_count: u32,
        to_composite: bool,
    },
    BeginComposite,
    EndComposite {
        render_mask: bool,
        using_mask: bool,
        id: InoxNodeUuid,
    },
}

impl DrawCommand {
    /// Given two draw commands, determine if they can be batched, and if so,
    /// construct a new draw command that encompasses both operations.
    ///
    /// If they cannot be batched, returns None.
    pub fn can_be_batched(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (
                Self::DrawPart {
                    render_mask: self_render_mask,
                    using_mask: self_using_mask,
                    stencil_reference: self_stencil_reference,
                    blend_mode: self_blend_mode,
                    indirect_offset: self_indirect_offset,
                    indirect_count: self_indirect_count,
                    to_composite: self_to_composite,
                },
                Self::DrawPart {
                    render_mask: other_render_mask,
                    using_mask: other_using_mask,
                    stencil_reference: other_stencil_reference,
                    blend_mode: other_blend_mode,
                    indirect_offset: other_indirect_offset,
                    indirect_count: other_indirect_count,
                    to_composite: other_to_composite,
                },
            ) => {
                let self_indirect_end = *self_indirect_offset
                    + (*self_indirect_count as usize
                        * std::mem::size_of::<wgpu::util::DrawIndexedIndirectArgs>())
                        as u64;

                if *self_render_mask == *other_render_mask
                    && *self_using_mask == *other_using_mask
                    && *self_stencil_reference == *other_stencil_reference
                    && *self_blend_mode == *other_blend_mode
                    && self_indirect_end == *other_indirect_offset
                    && *self_to_composite == *other_to_composite
                {
                    Some(Self::DrawPart {
                        render_mask: *self_render_mask,
                        using_mask: *self_using_mask,
                        stencil_reference: *self_stencil_reference,
                        blend_mode: *self_blend_mode,
                        indirect_offset: *self_indirect_offset,
                        indirect_count: *self_indirect_count + *other_indirect_count,
                        to_composite: *self_to_composite,
                    })
                } else {
                    None
                }
            }
            (_, _) => None,
        }
    }
}

#[derive(Default)]
pub struct DrawCommandList {
    commands: Vec<DrawCommand>,
}

impl DrawCommandList {
    fn batch_upsert(&mut self, item: DrawCommand) {
        if let Some(merged) = self.commands.last().and_then(|l| l.can_be_batched(&item)) {
            *self.commands.last_mut().unwrap() = merged;
        } else {
            self.commands.push(item);
        }
    }

    pub fn clear_current_stencil(&mut self, to_composite: bool) {
        self.batch_upsert(DrawCommand::ClearCurrentStencil { to_composite });
    }

    pub fn draw_part(
        &mut self,
        render_mask: bool,
        using_mask: bool,
        stencil_reference: u32,
        blend_mode: Option<wgpu::BlendState>,
        indirect_offset: wgpu::BufferAddress,
        indirect_count: u32,
        to_composite: bool,
    ) {
        self.batch_upsert(DrawCommand::DrawPart {
            render_mask,
            using_mask,
            stencil_reference,
            blend_mode,
            indirect_offset,
            indirect_count,
            to_composite,
        });
    }

    pub fn begin_composite(&mut self) {
        self.batch_upsert(DrawCommand::BeginComposite);
    }

    pub fn end_composite(&mut self, render_mask: bool, using_mask: bool, id: InoxNodeUuid) {
        self.batch_upsert(DrawCommand::EndComposite {
            render_mask,
            using_mask,
            id,
        });
    }

    fn flush_block(
        draw_session: &WgpuDrawSession<'_, '_>,
        puppet: &inox2d::puppet::Puppet,
        send: Sender<FlushResult>,
        thread_index: usize,
        commands: &[DrawCommand],
    ) {
        let viewports_config = &draw_session.viewports_config;
        let outputs = draw_session.render_target.outputs().unwrap();
        let composite = outputs.composite();
        let stencil = outputs.stencil();

        let masked_depthstencil = draw_session.resources.masked_depthstencil.clone();
        let mask_depthstencil = draw_session.resources.mask_depthstencil.clone();
        let ignore_depthstencil = draw_session.resources.ignore_depthstencil.clone();

        let indirect_buffer = draw_session.indirect_buffer.as_ref().unwrap();

        let num_layers = draw_session
            .render_target
            .color_target()
            .unwrap()
            .depth_or_array_layers();

        let mut encoder = draw_session
            .resources
            .create_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("DrawCommandList::flush"),
            });
        let mut binding_cache = draw_session.binding_cache.split();
        let mut render_pass: Option<wgpu::RenderPass<'_>> = None;
        let mut render_pass_state_set = false;
        let send = send.clone();

        #[cfg(feature = "timing")]
        let mut num_render_passes_constructed = 0;

        #[cfg(feature = "timing")]
        let start_time = std::time::Instant::now();

        // Issue an entirely separate set of drawing commands for each layer in
        // the target texture. We have to do this because wgpu's shader
        // translation is breaking our access to the View Index parameter.
        // The good news is, we'd have to do this anyway for atlassing, soooo
        for current_layer in 0..num_layers as usize {
            let multiview_mask = None;

            let color_view = outputs.color_target_layer_view(current_layer).unwrap();
            let stencil_view = outputs.stencil_layer_view(current_layer).unwrap();
            let composite_albedo_view = outputs.composite_albedo_layer_view(current_layer).unwrap();
            let composite_emissive_view = outputs
                .composite_emissive_layer_view(current_layer)
                .unwrap();
            let composite_bump_view = outputs.composite_bump_layer_view(current_layer).unwrap();
            let composite_stencil_view =
                outputs.composite_stencil_layer_view(current_layer).unwrap();

            for command in commands.iter() {
                match command {
                    DrawCommand::ClearCurrentStencil { to_composite: true } => {
                        if render_pass.is_none() {
                            render_pass = None; //Borrowck can't tell otherwise
                            composite.stencil().clear(encoder.encoder());
                        } else {
                            let render_pass = render_pass.as_mut().unwrap();
                            render_pass_state_set = false;
                            composite.stencil().clear_with_render_pass(
                                &draw_session.device,
                                render_pass,
                                &draw_session.resources,
                                &composite.as_color_attachments(), //NOTE: this does not actually write to these
                                multiview_mask,
                            );
                        }
                    }
                    DrawCommand::ClearCurrentStencil {
                        to_composite: false,
                    } => {
                        if render_pass.is_none() {
                            render_pass = None;
                            stencil.clear(encoder.encoder());
                        } else {
                            let render_pass = render_pass.as_mut().unwrap();
                            render_pass_state_set = false;
                            stencil.clear_with_render_pass(
                                &draw_session.device,
                                render_pass,
                                &draw_session.resources,
                                &[Some(color_view.as_color_attachment()), None, None],
                                multiview_mask,
                            );
                        }
                    }
                    DrawCommand::DrawPart {
                        render_mask,
                        using_mask,
                        stencil_reference,
                        blend_mode,
                        indirect_offset,
                        indirect_count,
                        to_composite,
                    } => {
                        let blend = [*blend_mode, *blend_mode, *blend_mode];

                        let surface_color_attach = Some(color_view.as_color_attachment());
                        let gbuffer_color = [
                            Some(composite_albedo_view.as_color_attachment()),
                            Some(composite_emissive_view.as_color_attachment()),
                            Some(composite_bump_view.as_color_attachment()),
                        ];
                        let unmasked_attach = [surface_color_attach, None, None];
                        let color_attachments = if *to_composite {
                            gbuffer_color.as_slice()
                        } else {
                            unmasked_attach.as_slice()
                        };

                        if render_pass.is_none() {
                            let depth_stencil_attachment = if *to_composite {
                                Some(composite_stencil_view.as_depth_stencil_attachment())
                            } else {
                                Some(stencil_view.as_depth_stencil_attachment())
                            };

                            #[cfg(feature = "timing")]
                            {
                                num_render_passes_constructed += 1;
                            }

                            drop(render_pass);

                            render_pass = Some(encoder.encoder().begin_render_pass(
                                &wgpu::RenderPassDescriptor {
                                    label: Some(&format!("{:?}", command)),
                                    color_attachments,
                                    depth_stencil_attachment,
                                    occlusion_query_set: None,
                                    timestamp_writes: None,
                                    multiview_mask,
                                },
                            ));

                            render_pass_state_set = false;
                        }

                        let render_pass = render_pass.as_mut().unwrap();

                        if !render_pass_state_set {
                            // NOTE: It seems like we could do this with
                            // the render pass once, but we actually have
                            // to reset these every draw call for whatever
                            // reason.
                            render_pass.set_vertex_buffer(
                                basic_vert::INPUT_INDEX_VERTS - 1,
                                draw_session.uploads.verts.slice(..),
                            );
                            render_pass.set_vertex_buffer(
                                basic_vert::INPUT_INDEX_UVS - 1,
                                draw_session.uploads.uvs.slice(..),
                            );
                            render_pass.set_vertex_buffer(
                                basic_vert::INPUT_INDEX_DEFORM - 1,
                                draw_session.uploads.deforms.slice(..),
                            );
                            render_pass.set_index_buffer(
                                draw_session.uploads.indices.slice(..),
                                wgpu::IndexFormat::Uint32,
                            );

                            let (vert_binding, viewport_binding) = binding_cache.bind_basic_vert(
                                &*draw_session.resources,
                                draw_session.basic_vert_buffer.as_ref().unwrap(),
                                viewports_config,
                            );

                            render_pass.set_bind_group(0, Some(&vert_binding), &[]);
                            render_pass.set_bind_group(
                                2,
                                Some(&viewport_binding),
                                &[(current_layer * Viewport::static_size()) as u32],
                            );
                        }

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

                        render_pass.set_stencil_reference(*stencil_reference);

                        if *render_mask {
                            //TODO: What happens if a mask is also masked?
                            let texture_views: Vec<_> = draw_session
                                .uploads
                                .model_textures
                                .iter()
                                .map(|mt| mt.view())
                                .collect();
                            let (frag_binding, viewport_frag_binding) = binding_cache
                                .bind_basic_mask_frag(
                                    &draw_session.resources,
                                    texture_views.as_slice(),
                                    draw_session.basic_mask_frag_buffer.as_ref().unwrap(),
                                    viewports_config,
                                );
                            let pipeline = draw_session
                                .resources
                                .part_mask_pipeline_with_configuration(
                                    &draw_session.device,
                                    formats,
                                    blend,
                                    [
                                        wgpu::ColorWrites::empty(),
                                        wgpu::ColorWrites::empty(),
                                        wgpu::ColorWrites::empty(),
                                    ],
                                    Some(mask_depthstencil.clone()),
                                    multiview_mask,
                                );
                            render_pass.set_pipeline(pipeline.pipeline());
                            render_pass.set_bind_group(1, Some(&frag_binding), &[]);
                            render_pass.set_bind_group(
                                3,
                                Some(&viewport_frag_binding),
                                &[(current_layer * Viewport::static_size()) as u32],
                            );
                        } else {
                            //Regular parts
                            let all = wgpu::ColorWrites::ALL;
                            let texture_views: Vec<_> = draw_session
                                .uploads
                                .model_textures
                                .iter()
                                .map(|mt| mt.view())
                                .collect();
                            let (frag_binding, viewport_frag_binding) = binding_cache
                                .bind_basic_frag(
                                    &draw_session.resources,
                                    &texture_views,
                                    draw_session.basic_frag_buffer.as_ref().unwrap(),
                                    viewports_config,
                                );

                            let pipeline = if *using_mask {
                                draw_session.resources.part_pipeline_with_configuration(
                                    &draw_session.device,
                                    formats,
                                    blend,
                                    [all, all, all],
                                    Some(masked_depthstencil.clone()),
                                    multiview_mask,
                                )
                            } else {
                                draw_session.resources.part_pipeline_with_configuration(
                                    &draw_session.device,
                                    formats,
                                    blend,
                                    [all, all, all],
                                    Some(ignore_depthstencil.clone()),
                                    multiview_mask,
                                )
                            };

                            render_pass.set_pipeline(pipeline.pipeline());
                            render_pass.set_bind_group(1, Some(&frag_binding), &[]);
                            render_pass.set_bind_group(
                                3,
                                Some(&viewport_frag_binding),
                                &[(current_layer * Viewport::static_size()) as u32],
                            );
                        }

                        render_pass.multi_draw_indexed_indirect(
                            indirect_buffer,
                            *indirect_offset,
                            *indirect_count,
                        );
                    }
                    DrawCommand::BeginComposite => {
                        render_pass = None;
                        composite.clear(encoder.encoder());
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

                        let depth_stencil_attachment =
                            Some(stencil_view.as_depth_stencil_attachment());

                        //TODO: Do we even want blending on in Normal mode?
                        let blend = Some(blend_mode_to_state(components.drawable.blending.mode));

                        let color_attachments =
                            [Some(color_view.as_color_attachment()), None, None];

                        // Strictly speaking, we will never be able to batch the end of a
                        // composite operation. (Or so I think?!)
                        render_pass = None;
                        let mut render_pass =
                            encoder
                                .encoder()
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
                            basic_vert::INPUT_INDEX_VERTS - 1,
                            draw_session.uploads.verts.slice(..),
                        );
                        render_pass.set_vertex_buffer(
                            basic_vert::INPUT_INDEX_UVS - 1,
                            draw_session.uploads.uvs.slice(..),
                        );
                        render_pass.set_vertex_buffer(
                            basic_vert::INPUT_INDEX_DEFORM - 1,
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
                            let viewport_binding = binding_cache
                                .bind_composite_vert(&draw_session.resources, viewports_config);
                            let (frag_binding, viewport_frag_binding) = binding_cache
                                .bind_composite_frag(
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
                            //NOTE composite.vert does not use bindgroup 0.
                            render_pass.set_bind_group(0, None, &[]);
                            render_pass.set_bind_group(
                                1,
                                Some(&frag_binding),
                                &[index.composite_frag.unwrap() as u32],
                            );
                            render_pass.set_bind_group(
                                2,
                                Some(&viewport_binding),
                                &[(current_layer * Viewport::static_size()) as u32],
                            );
                            render_pass.set_bind_group(
                                3,
                                Some(&viewport_frag_binding),
                                &[(current_layer * Viewport::static_size()) as u32],
                            );
                            render_pass.draw_indexed(0..6, 0, 0..1); //TODO: Where do these vertices come from!?!?
                        }
                    }
                }
            }

            render_pass = None;
        }

        #[cfg(feature = "timing")]
        send.send({
            let total_time = std::time::Instant::now() - start_time;

            (
                thread_index,
                draw_session.resources.finish_encoding(encoder),
                binding_cache.untether(),
                num_render_passes_constructed,
                total_time,
            )
        })
        .unwrap();

        #[cfg(not(feature = "timing"))]
        send.send({
            (
                thread_index,
                draw_session.resources.finish_encoding(encoder),
                binding_cache.untether(),
            )
        })
        .unwrap();
    }

    pub fn flush(
        draw_session: &mut WgpuDrawSession<'_, '_>,
        puppet: &inox2d::puppet::Puppet,
    ) -> Option<wgpu::SubmissionIndex> {
        let optimal_chunk_size = max(
            draw_session.draw_commands.commands.len() / current_num_threads() * 2,
            1,
        );
        let (send, recv) = bounded(0);

        #[cfg(feature = "timing")]
        eprintln!(
            "      (Commands: {})",
            draw_session.draw_commands.commands.len()
        );

        let (_, (results, submission_index)) = rayon::join(
            || {
                draw_session
                    .draw_commands
                    .commands
                    .par_chunks(optimal_chunk_size)
                    .enumerate()
                    .map(|(thread_index, commands)| {
                        let send = send.clone();
                        Self::flush_block(draw_session, puppet, send, thread_index, commands);
                    })
                    .collect::<Vec<_>>();

                drop(send);
            },
            || {
                let mut next_index = 0;
                let mut unprocessed = HashMap::new();
                let mut results = vec![];
                let mut submission_index = None;

                for back in recv.iter() {
                    let thread_index = back.0;

                    unprocessed.insert(thread_index, back);

                    while let Some(process) = unprocessed.remove(&next_index) {
                        next_index += 1;
                        let buffer = process.1;

                        submission_index =
                            Some(draw_session.resources.queue.submit(std::iter::once(buffer)));

                        #[cfg(not(feature = "timing"))]
                        results.push((process.0, process.2));

                        #[cfg(feature = "timing")]
                        results.push((process.0, process.2, process.3, process.4));
                    }
                }

                while let Some(process) = unprocessed.remove(&next_index) {
                    eprintln!("Took {}", process.0);
                    next_index += 1;
                    let buffer = process.1;

                    submission_index =
                        Some(draw_session.resources.queue.submit(std::iter::once(buffer)));

                    #[cfg(not(feature = "timing"))]
                    results.push((process.0, process.2));

                    #[cfg(feature = "timing")]
                    results.push((process.0, process.2, process.3, process.4));
                }

                assert!(unprocessed.len() == 0);

                (results, submission_index)
            },
        );

        #[cfg(feature = "timing")]
        WgpuDrawSession::lap_time("Encode & Submit", &mut draw_session.last_lap_time);

        #[cfg(feature = "timing")]
        let mut bind_count = (0, 0, 0, 0, 0);

        let num_layers = draw_session
            .render_target
            .color_target()
            .unwrap()
            .depth_or_array_layers();

        for back in results {
            #[cfg(not(feature = "timing"))]
            let (thread_index, binding_cache) = back;

            #[cfg(feature = "timing")]
            let (thread_index, binding_cache, num_render_passes_constructed, total_time) = back;

            #[cfg(feature = "timing")]
            {
                eprintln!(
                    "      (Encode thread {} time: {})",
                    thread_index,
                    total_time.as_micros() as f32 / 1000.0
                );
                let this_bind_count = binding_cache.delta_len();

                bind_count.0 += this_bind_count.0;
                bind_count.1 += this_bind_count.1;
                bind_count.2 += this_bind_count.2;
                bind_count.3 += this_bind_count.3;
                bind_count.4 += this_bind_count.4;

                if num_render_passes_constructed > num_layers {
                    eprintln!(
                        "        ({} RenderPass constructions...)",
                        num_render_passes_constructed
                    );
                }
            }

            draw_session.binding_cache.merge(binding_cache);
        }

        #[cfg(feature = "timing")]
        {
            if bind_count.0 > 0 {
                eprintln!("      ({} basic_vert BindGroup creations...)", bind_count.0);
            }

            if bind_count.1 > 0 {
                eprintln!("      ({} basic_frag BindGroup creations...)", bind_count.1);
            }

            if bind_count.2 > 0 {
                eprintln!(
                    "      ({} basic_mask_frag BindGroup creations...)",
                    bind_count.2
                );
            }

            if bind_count.3 > 0 {
                eprintln!(
                    "      ({} composite_vert BindGroup creations...)",
                    bind_count.3
                );
            }

            if bind_count.4 > 0 {
                eprintln!(
                    "      ({} composite_frag BindGroup creations...)",
                    bind_count.4
                );
            }

            WgpuDrawSession::lap_time("Collate", &mut draw_session.last_lap_time);
        }

        draw_session.draw_commands.commands.drain(..);

        #[cfg(feature = "timing")]
        WgpuDrawSession::lap_time("Queue drain", &mut draw_session.last_lap_time);

        submission_index
    }
}
