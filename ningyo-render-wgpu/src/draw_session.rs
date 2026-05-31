use glam::Mat4;
use glam::UVec2;
use inox2d::node::components::MaskMode;
use inox2d::node::drawables::DrawableKind;
use inox2d::node::{InoxNodeUuid, components, drawables};
use inox2d::render::CompositeRenderCtx;
//hey wait a second that's just a u32 newtype! UUIDs are four of those!
use inox2d::render::{self, DrawSession};
use ningyo_extensions::CurrentSurfaceTextureExt;
use std::error::Error;
use wgpu;

use crate::WgpuRenderer;
use crate::buffer_builder::BufferBuilder;
use crate::draw_command::DrawCommandList;
use crate::shaders::basic::{basic_frag, basic_mask_frag, basic_vert, composite_frag};
use crate::texture::{DepthStencilTexture, GBuffer};

use std::collections::HashMap;

use crate::binding_cache::BindingCache;
use crate::renderer::BufferIndices;
use crate::resources::WgpuResources;
use crate::uploads::WgpuUploads;

pub struct WgpuDrawSession<'a> {
    /// The rendering resources for this draw session.
    ///
    /// We keep the resources locked throughout the draw session to avoid
    /// contention between multiple renderers.
    pub(crate) resources: &'a WgpuResources,

    /// The uploads for the particular model that we will be drawing.
    pub(crate) uploads: &'a WgpuUploads,

    /// All textures used as render targets, excluding the surface color
    /// buffer.
    ///
    /// GBuffer is used solely for composite rendering, where rendered pixels
    /// are used for a deferred shading pass.
    pub(crate) render_targets: &'a mut Option<(GBuffer, DepthStencilTexture)>,

    pub(crate) buffer_indices: &'a mut HashMap<u32, BufferIndices>,
    pub(crate) builder_basic_vert: &'a mut BufferBuilder<basic_vert::Input>,
    pub(crate) builder_basic_frag: &'a mut BufferBuilder<basic_frag::Input>,
    pub(crate) builder_basic_mask_frag: &'a mut BufferBuilder<basic_mask_frag::Input>,
    pub(crate) builder_composite_frag: &'a mut BufferBuilder<composite_frag::Input>,

    pub(crate) binding_cache: &'a mut BindingCache,

    /// Local clone of the device (to avoid overlapping borrows.)
    pub(crate) device: wgpu::Device,

    /// The drawing session's command encoder.
    pub(crate) encoder: wgpu::CommandEncoder,

    /// The output texture to render.
    pub(crate) view: wgpu::TextureView,

    /// The position of the root of our model.
    viewmatrix: Mat4,

    /// All of the node names (for debugging purposes).
    pub(crate) node_names: HashMap<InoxNodeUuid, String>,

    /// The currently active set of vert shader uniforms.
    pub(crate) basic_vert_buffer: Option<wgpu::Buffer>,

    /// The currently active set of frag shader uniforms (for non-mask usage)
    pub(crate) basic_frag_buffer: Option<wgpu::Buffer>,

    /// The currently active set of frag shader uniforms (for mask usage)
    pub(crate) basic_mask_frag_buffer: Option<wgpu::Buffer>,

    /// The currently active set of composite deferred pass uniforms
    pub(crate) composite_frag_buffer: Option<wgpu::Buffer>,

    pub(crate) draw_commands: &'a mut DrawCommandList,

    pub(crate) last_submission_index: &'a mut Option<wgpu::SubmissionIndex>,

    pub(crate) viewports_config: &'a wgpu::Buffer,

    last_mask_threshold: f32,
    is_in_mask: bool,
    is_in_composite: bool,
    stencil_reference_value: u32,

    last_mask: Vec<(InoxNodeUuid, MaskMode)>,

    #[cfg(feature = "tracy")]
    encoder_query: wgpu_profiler::GpuProfilerQuery,
}

impl<'a> WgpuDrawSession<'a> {
    pub fn begin(
        renderer: &'a mut WgpuRenderer<'_>,
        puppet: &inox2d::puppet::Puppet,
    ) -> Result<Self, Box<dyn Error>> {
        if renderer.render_targets.is_none() {
            panic!("Buffer is not yet set up.");
        }

        let resources = &*renderer.resources;

        #[allow(unused_mut)]
        let mut encoder =
            resources
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Inox2DWGPU"),
                });

        #[cfg(feature = "tracy")]
        let encoder_query = resources.start_query(&mut encoder);

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
            last_mask: Vec::new(),
            basic_vert_buffer: None,
            basic_frag_buffer: None,
            basic_mask_frag_buffer: None,
            composite_frag_buffer: None,
            draw_commands: &mut renderer.draw_commands,
            binding_cache: &mut renderer.bind_cache,
            last_submission_index: &mut renderer.last_submission_index,
            viewports_config: &renderer.viewports_config.1,

            #[cfg(feature = "tracy")]
            encoder_query,
        };

        session.buffer_prepass(puppet);

        Ok(session)
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

        self.basic_vert_buffer = Some(
            self.builder_basic_vert
                .commit(&self.device, &self.resources.queue),
        );
        self.basic_frag_buffer = Some(
            self.builder_basic_frag
                .commit(&self.device, &self.resources.queue),
        );
        self.basic_mask_frag_buffer = Some(
            self.builder_basic_mask_frag
                .commit(&self.device, &self.resources.queue),
        );
        self.composite_frag_buffer = Some(
            self.builder_composite_frag
                .commit(&self.device, &self.resources.queue),
        );

        self.last_mask_threshold = 0.0;
        self.last_mask = vec![];
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
            self.last_mask_threshold = masks.threshold.clamp(0.0, 1.0);
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
        let mut masks_are_equal = masks.threshold == self.last_mask_threshold
            && masks.masks.len() == self.last_mask.len();
        for (mask_a, mask_b) in masks.masks.iter().zip(self.last_mask.iter()) {
            if !masks_are_equal {
                break;
            }

            masks_are_equal &= mask_a.mode == mask_b.1 && mask_a.source == mask_b.0;
        }

        if !masks_are_equal {
            self.draw_commands.clear_current_stencil();

            self.last_mask_threshold = masks.threshold;
            self.last_mask = masks
                .masks
                .iter()
                .map(|mask| {
                    (
                        mask.source,
                        match mask.mode {
                            MaskMode::Mask => MaskMode::Mask,
                            MaskMode::Dodge => MaskMode::Dodge,
                        },
                    )
                })
                .collect();
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
        _components: &drawables::TexturedMeshComponents,
        render_ctx: &render::TexturedMeshRenderCtx,
        id: InoxNodeUuid,
    ) {
        self.draw_commands.draw_part(
            render_mask,
            self.is_in_mask,
            self.stencil_reference_value,
            id,
            render_ctx,
        );
    }

    fn begin_composite_content(
        &mut self,
        _as_mask: bool,
        _components: &drawables::CompositeComponents,
        _render_ctx: &render::CompositeRenderCtx,
        _id: InoxNodeUuid,
    ) {
        self.draw_commands.begin_composite();
        self.is_in_composite = true;
    }

    fn finish_composite_content(
        &mut self,
        render_mask: bool,
        _components: &drawables::CompositeComponents,
        _render_ctx: &render::CompositeRenderCtx,
        id: InoxNodeUuid,
    ) {
        self.draw_commands
            .end_composite(render_mask, self.is_in_mask, id);
    }

    fn on_end_draw(mut self, puppet: &inox2d::puppet::Puppet) {
        DrawCommandList::flush(&mut self, puppet);

        #[cfg(feature = "tracy")]
        {
            self.resources
                .end_query(&mut self.encoder, self.encoder_query);
        }

        let end = self.encoder.finish();
        *self.last_submission_index = Some(self.resources.queue.submit(std::iter::once(end)));
    }
}
