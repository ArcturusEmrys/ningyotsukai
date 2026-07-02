use inox2d::node::components::MaskMode;
use inox2d::node::drawables::DrawableKind;
use inox2d::node::{InoxNodeUuid, components, drawables};
use inox2d::render::CompositeRenderCtx;
//hey wait a second that's just a u32 newtype! UUIDs are four of those!
use inox2d::render::{self, DrawSession};
use std::error::Error;
use std::sync::MutexGuard;
use wgpu;

use crate::buffer_builder::BufferBuilder;
use crate::camera::CameraExt;
use crate::draw_command::DrawCommandList;
use crate::shaders::basic::{basic_frag, basic_mask_frag, basic_vert, composite_frag};
use crate::{RenderTarget, WgpuRenderer};

use std::collections::HashMap;

use crate::binding_cache::BindingCache;
use crate::renderer::BufferIndices;
use crate::resources::WgpuResources;
use crate::uploads::WgpuUploads;

pub struct WgpuDrawSession<'a, 'window> {
    /// The rendering resources for this draw session.
    ///
    /// We keep the resources locked throughout the draw session to avoid
    /// contention between multiple renderers.
    pub(crate) resources: &'a WgpuResources,

    /// The uploads for the particular model that we will be drawing.
    pub(crate) uploads: &'a mut WgpuUploads,

    pub(crate) buffer_indices: &'a mut HashMap<u32, BufferIndices>,
    pub(crate) builder_basic_vert: &'a mut BufferBuilder<basic_vert::Input>,
    pub(crate) builder_basic_frag: &'a mut BufferBuilder<basic_frag::Input>,
    pub(crate) builder_basic_mask_frag: &'a mut BufferBuilder<basic_mask_frag::Input>,
    pub(crate) builder_composite_frag: &'a mut BufferBuilder<composite_frag::Input>,

    pub(crate) binding_cache: &'a mut BindingCache<'static>,

    /// Local clone of the device (to avoid overlapping borrows.)
    pub(crate) device: wgpu::Device,

    /// The current render target.
    pub(crate) render_target: MutexGuard<'a, RenderTarget<'window>>,

    /// The renderer's camera position as an artboard-space matrix
    pub(crate) artboard_matrix: glam::Mat4,

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

    pub(crate) viewports_config: wgpu::Buffer,

    last_mask_threshold: f32,
    is_in_mask: bool,
    is_in_composite: bool,
    stencil_reference_value: u32,

    last_mask: Vec<(InoxNodeUuid, MaskMode)>,

    #[cfg(feature = "timing")]
    pub start_time: std::time::Instant,

    #[cfg(feature = "timing")]
    pub last_lap_time: std::time::Instant,
}

impl<'a, 'window> WgpuDrawSession<'a, 'window> {
    pub fn begin(
        renderer: &'a mut WgpuRenderer<'window>,
        puppet: &inox2d::puppet::Puppet,
    ) -> Result<Self, Box<dyn Error>> {
        #[cfg(feature = "timing")]
        let start_time = std::time::Instant::now();

        let resources = &*renderer.resources;

        let render_target = renderer.render_target.lock().unwrap();

        let node_names = puppet
            .nodes()
            .iter()
            .map(|n| (n.uuid, n.name.clone()))
            .collect::<HashMap<_, _>>();
        let artboard_matrix = renderer.camera.to_artboard_matrix();

        renderer.uploads.update_deforms(puppet, resources)?;

        let device = resources.device.clone();

        let mut session = WgpuDrawSession {
            resources,
            uploads: &mut renderer.uploads,
            buffer_indices: &mut renderer.buffer_indices,
            builder_basic_vert: &mut renderer.builder_basic_vert,
            builder_basic_frag: &mut renderer.builder_basic_frag,
            builder_basic_mask_frag: &mut renderer.builder_basic_mask_frag,
            builder_composite_frag: &mut renderer.builder_composite_frag,
            device,
            viewports_config: render_target.viewports_config()?.clone(),
            render_target,
            artboard_matrix,
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

            #[cfg(feature = "timing")]
            last_lap_time: start_time.clone(),

            #[cfg(feature = "timing")]
            start_time,
        };

        session.buffer_prepass(puppet);

        #[cfg(feature = "timing")]
        session.lap("Buffer prepass");

        Ok(session)
    }

    #[cfg(feature = "timing")]
    pub fn lap_time(feature: &str, last_lap_time: &mut std::time::Instant) {
        let this_lap_time = std::time::Instant::now();
        let duration = this_lap_time - *last_lap_time;

        *last_lap_time = this_lap_time;

        eprintln!(
            "    {}: {}ms",
            feature,
            duration.as_micros() as f32 / 1000.0
        );
    }

    #[cfg(feature = "timing")]
    pub fn lap(&mut self, feature: &str) {
        Self::lap_time(feature, &mut self.last_lap_time)
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
                        mvp: (self.artboard_matrix * *components.transform).to_cols_array_2d(),
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

impl<'a, 'window> DrawSession<'a> for WgpuDrawSession<'a, 'window> {
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
            self.draw_commands
                .clear_current_stencil(self.is_in_composite);

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
            self.is_in_composite,
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
        self.is_in_composite = false;
        self.draw_commands
            .end_composite(render_mask, self.is_in_mask, id);
    }

    fn on_end_draw(mut self, puppet: &inox2d::puppet::Puppet) {
        #[cfg(feature = "timing")]
        self.lap("Tree walk");

        *self.last_submission_index = DrawCommandList::flush(&mut self, puppet);

        #[cfg(feature = "timing")]
        self.lap("Misc");

        #[cfg(feature = "timing")]
        {
            let this_lap_time = std::time::Instant::now();
            let total_time = this_lap_time - self.start_time;
            eprintln!("    (Total): {}ms", total_time.as_micros() as f32 / 1000.0);
        }
    }
}
