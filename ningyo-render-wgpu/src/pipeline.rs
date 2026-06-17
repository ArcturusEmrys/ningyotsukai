use wgpu;

use crate::shader::{FragmentShader, VertexShader};
use std::cmp::max;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::num::NonZero;

#[derive(Clone, Debug)]
pub struct Pipeline<V, F>
where
    V: VertexShader,
    F: FragmentShader,
{
    pipeline: wgpu::RenderPipeline,
    phantom_vert: PhantomData<V>,
    phantom_frag: PhantomData<F>,
}

impl<V, F> Pipeline<V, F>
where
    V: VertexShader,
    F: FragmentShader,
{
    pub fn new(
        device: &wgpu::Device,
        vert: &V,
        frag: &F,
        formats: F::TargetArray<Option<wgpu::TextureFormat>>,
        blend: F::TargetArray<Option<wgpu::BlendState>>,
        write_mask: F::TargetArray<wgpu::ColorWrites>,
        depth_stencil: Option<wgpu::DepthStencilState>,
        multiview_mask: Option<NonZero<u32>>,
    ) -> Self {
        let vert_bindgroup_layouts = vert.bindgroup_layout();
        let frag_bindgroup_layouts = frag.bindgroup_layout();
        let mut mixed = vec![None; max(vert_bindgroup_layouts.len(), frag_bindgroup_layouts.len())];

        for (index, fbl) in frag_bindgroup_layouts.iter().enumerate() {
            if fbl.is_some() {
                mixed[index] = fbl.as_ref();
            }
        }

        for (index, vbl) in vert_bindgroup_layouts.iter().enumerate() {
            // NOTE: We actually could share bindings across shaders, the
            // problem is that our shader codegen isn't aware of this, so it
            // claims the binding is only visible to one stage or the other.
            // This really should be a property of the pipeline, but in order
            // to do that, I need the codegen tool to add a method to get the
            // bindgroup layout descriptors so we can create them here instead.
            // Also, all the bindings the shaders generate have to have two
            // sets of visibility flags.
            if mixed[index].is_some() && vbl.is_some() {
                panic!("Cannot share binding {} across shaders!", index);
            }

            if vbl.is_some() {
                mixed[index] = vbl.as_ref();
            }
        }

        let name = format!("Pipeline of {} + {}", vert.label(), frag.label());
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&name),
            bind_group_layouts: mixed.as_slice(),
            immediate_size: 0,
        });

        let mut fragment_targets = frag.preferred_color_targets();
        for (index, (format, (blend, write_mask))) in formats
            .into_iter()
            .zip(blend.into_iter().zip(write_mask.into_iter()))
            .enumerate()
        {
            let target = fragment_targets.as_mut()[index].as_mut().expect(
                "Excess color attachment options were provided to the pipeline constructor",
            );
            if let Some(format) = format {
                target.format = format;
                target.blend = blend;
                target.write_mask = write_mask;
            } else {
                //Format NONE means the color target wasn't provided, so erase it.
                fragment_targets.as_mut()[index] = None;
            }
        }

        let fragment = frag.as_fragment_state(&fragment_targets.as_ref());

        Self {
            pipeline: device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&name),
                layout: Some(&layout),
                vertex: vert.as_vertex_state(),
                fragment: Some(fragment),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: true,
                    ..Default::default()
                },
                depth_stencil,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask,
                cache: None,
            }),
            phantom_frag: PhantomData::default(),
            phantom_vert: PhantomData::default(),
        }
    }

    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }
}

/// Cache for different pipelines with the same shader program.
///
/// Necessary as certain configurations cannot be changed dynamically in WGPU.
#[derive(Debug)]
pub struct PipelineGroup<V, F>
where
    V: VertexShader,
    F: FragmentShader,
{
    vert: V,
    frag: F,
    cache: HashMap<
        (
            F::TargetArray<Option<wgpu::TextureFormat>>,
            F::TargetArray<Option<wgpu::BlendState>>,
            F::TargetArray<wgpu::ColorWrites>,
            Option<wgpu::DepthStencilState>,
            Option<NonZero<u32>>,
        ),
        Pipeline<V, F>,
    >,
}

impl<V, F> PipelineGroup<V, F>
where
    V: VertexShader,
    F: FragmentShader,
{
    pub fn new(vert: V, frag: F) -> Self {
        Self {
            vert,
            frag,
            cache: HashMap::new(),
        }
    }

    /// Yields a pipeline with the chosen configuration iff it has already been
    /// created.
    ///
    /// Guaranteed to not allocate a new pipeline, but may fail if the chosen
    /// configuration does not already exist.
    pub fn with_configuration_cached(
        &self,
        formats: F::TargetArray<Option<wgpu::TextureFormat>>,
        blend: F::TargetArray<Option<wgpu::BlendState>>,
        write_mask: F::TargetArray<wgpu::ColorWrites>,
        depth_stencil: Option<wgpu::DepthStencilState>,
        multiview_mask: Option<NonZero<u32>>,
    ) -> Option<Pipeline<V, F>> {
        self.cache
            .get(&(formats, blend, write_mask, depth_stencil, multiview_mask))
            .cloned()
    }

    /// Yields a pipeline with the chosen configuration.
    pub fn with_configuration(
        &mut self,
        device: &wgpu::Device,
        formats: F::TargetArray<Option<wgpu::TextureFormat>>,
        blend: F::TargetArray<Option<wgpu::BlendState>>,
        write_mask: F::TargetArray<wgpu::ColorWrites>,
        depth_stencil: Option<wgpu::DepthStencilState>,
        multiview_mask: Option<NonZero<u32>>,
    ) -> Pipeline<V, F> {
        self.cache
            .entry((formats, blend, write_mask, depth_stencil, multiview_mask))
            .or_insert_with_key(
                |(formats, blend, write_mask, depth_stencil, multiview_mask)| {
                    Pipeline::new(
                        device,
                        &self.vert,
                        &self.frag,
                        formats.clone(),
                        blend.clone(),
                        write_mask.clone(),
                        depth_stencil.clone(),
                        *multiview_mask,
                    )
                },
            )
            .clone()
    }
}
