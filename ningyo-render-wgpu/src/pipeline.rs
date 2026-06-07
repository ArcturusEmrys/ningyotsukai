use wgpu;

use crate::shader::{FragmentShader, VertexShader};
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
        multiview_mask: Option<NonZero<u32>>
    ) -> Self {
        let name = format!("Pipeline of {} + {}", vert.label(), frag.label());
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&name),

            // NOTE: This assumes vertex shaders always use set 0 and fragment shaders always use set 1.
            bind_group_layouts: &[Some(vert.bindgroup_layout()), Some(frag.bindgroup_layout())],
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

    pub fn bind_vertex<'a, BG>(
        &self,
        render_pass: &mut wgpu::RenderPass,
        bind_group: BG,
        offsets: &[wgpu::DynamicOffset],
    ) where
        Option<&'a wgpu::BindGroup>: From<BG>,
    {
        render_pass.set_bind_group(0, bind_group, offsets)
    }

    pub fn bind_frag<'a, BG>(
        &self,
        render_pass: &mut wgpu::RenderPass,
        bind_group: BG,
        offsets: &[wgpu::DynamicOffset],
    ) where
        Option<&'a wgpu::BindGroup>: From<BG>,
    {
        render_pass.set_bind_group(1, bind_group, offsets)
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
            .or_insert_with_key(|(formats, blend, write_mask, depth_stencil, multiview_mask)| {
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
            })
            .clone()
    }
}
