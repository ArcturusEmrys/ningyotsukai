use std::num::NonZero;

use crate::pipeline;
use crate::shader::FragmentShader;
use crate::shaders::basic::{
    basic_frag, basic_mask_frag, basic_vert, composite_frag, composite_vert,
};
use crate::shaders::{mipmap_gen_frag, mipmap_gen_vert, null_frag};

/// Storage area for pipelines we need to use during rendering.
///
/// This has nothing to do with wgpu::PipelineCache.
#[derive(Debug)]
pub struct PipelineCache<'a> {
    tether: Option<&'a PipelineCache<'a>>,

    part_pipeline: pipeline::PipelineGroup<basic_vert::Shader, basic_frag::Shader>,
    part_mask_pipeline: pipeline::PipelineGroup<basic_vert::Shader, basic_mask_frag::Shader>,
    composite_pipeline: pipeline::PipelineGroup<composite_vert::Shader, composite_frag::Shader>,
    clear_pipeline: pipeline::PipelineGroup<mipmap_gen_vert::Shader, null_frag::Shader>,
    mipmap_gen_pipeline: pipeline::PipelineGroup<mipmap_gen_vert::Shader, mipmap_gen_frag::Shader>,
}

impl PipelineCache<'static> {
    pub fn new(
        part_shader_vert: &basic_vert::Shader,
        part_shader_frag: &basic_frag::Shader,
        part_shader_mask_frag: &basic_mask_frag::Shader,
        composite_shader_vert: &composite_vert::Shader,
        composite_shader_frag: &composite_frag::Shader,
        mipmap_gen_vert: &mipmap_gen_vert::Shader,
        mipmap_gen_frag: &mipmap_gen_frag::Shader,
        null_frag: &null_frag::Shader,
    ) -> Self {
        //TODO: We need a pipeline per Inochi blending mode
        //(or some kind of ubershader blending)

        let part_pipeline =
            pipeline::PipelineGroup::new(part_shader_vert.clone(), part_shader_frag.clone());
        let part_mask_pipeline =
            pipeline::PipelineGroup::new(part_shader_vert.clone(), part_shader_mask_frag.clone());
        let composite_pipeline = pipeline::PipelineGroup::new(
            composite_shader_vert.clone(),
            composite_shader_frag.clone(),
        );
        let mipmap_gen_pipeline =
            pipeline::PipelineGroup::new(mipmap_gen_vert.clone(), mipmap_gen_frag.clone());
        let clear_pipeline =
            pipeline::PipelineGroup::new(mipmap_gen_vert.clone(), null_frag.clone());

        Self {
            tether: None,
            part_pipeline,
            part_mask_pipeline,
            composite_pipeline,
            mipmap_gen_pipeline,
            clear_pipeline,
        }
    }
}

impl<'a> PipelineCache<'a> {
    /// Create a subsidiary PipelineCache that borrows data from another one.
    ///
    /// Pipelines stored in the borrowed pipeline cache will be accessible in
    /// the returned cache, but any new data will be stored in the subsidiary
    /// cache.You will need to merge the cache items back into the parent in
    /// order to retain them.
    ///
    /// This is primarily intended for multithreaded rendering scenarios; you
    /// can create multiple split pipeline caches and then merge them later.
    /// You can also use this for immutable access to a single binding cache.
    pub fn split<'b>(&'b self) -> PipelineCache<'b>
    where
        'b: 'a,
    {
        Self {
            tether: Some(self),
            part_pipeline: self.part_pipeline.split(),
            part_mask_pipeline: self.part_mask_pipeline.split(),
            composite_pipeline: self.composite_pipeline.split(),
            clear_pipeline: self.clear_pipeline.split(),
            mipmap_gen_pipeline: self.mipmap_gen_pipeline.split(),
        }
    }

    /// Remove connection to the borrowed binding cache and its lifetime.
    pub fn untether(self) -> PipelineCache<'static> {
        // Like with BindingCache, pulling all the owned variables out of a
        // struct won't erase the lifetime of the borrowed portion. However,
        // unlike that struct, we don't have a reasonable Default impl!
        // Instead we have to clone all the pipeline groups.
        PipelineCache {
            tether: None,
            part_pipeline: self.part_pipeline.clone(),
            part_mask_pipeline: self.part_mask_pipeline.clone(),
            composite_pipeline: self.composite_pipeline.clone(),
            clear_pipeline: self.clear_pipeline.clone(),
            mipmap_gen_pipeline: self.mipmap_gen_pipeline.clone(),
        }
    }

    pub fn clear_pipeline_with_configuration(
        &mut self,
        device: &wgpu::Device,
        formats: <null_frag::Shader as FragmentShader>::TargetArray<Option<wgpu::TextureFormat>>,
        blend: <null_frag::Shader as FragmentShader>::TargetArray<Option<wgpu::BlendState>>,
        write_mask: <null_frag::Shader as FragmentShader>::TargetArray<wgpu::ColorWrites>,
        depth_stencil: Option<wgpu::DepthStencilState>,
        multiview_mask: Option<NonZero<u32>>,
    ) -> pipeline::Pipeline<mipmap_gen_vert::Shader, null_frag::Shader> {
        if let Some(tether) = self.tether {
            if let Some(premade_entry) = tether.clear_pipeline.with_configuration_cached(
                formats,
                blend,
                write_mask,
                depth_stencil.clone(),
                multiview_mask,
            ) {
                return premade_entry;
            }
        }

        self.clear_pipeline.with_configuration(
            device,
            formats,
            blend,
            write_mask,
            depth_stencil,
            multiview_mask,
        )
    }

    pub fn mipmap_gen_pipeline_with_configuration(
        &mut self,
        device: &wgpu::Device,
        formats: <mipmap_gen_frag::Shader as FragmentShader>::TargetArray<
            Option<wgpu::TextureFormat>,
        >,
        blend: <mipmap_gen_frag::Shader as FragmentShader>::TargetArray<Option<wgpu::BlendState>>,
        write_mask: <mipmap_gen_frag::Shader as FragmentShader>::TargetArray<wgpu::ColorWrites>,
        depth_stencil: Option<wgpu::DepthStencilState>,
        multiview_mask: Option<NonZero<u32>>,
    ) -> pipeline::Pipeline<mipmap_gen_vert::Shader, mipmap_gen_frag::Shader> {
        if let Some(tether) = self.tether {
            if let Some(premade_entry) = tether.mipmap_gen_pipeline.with_configuration_cached(
                formats,
                blend,
                write_mask,
                depth_stencil.clone(),
                multiview_mask,
            ) {
                return premade_entry;
            }
        }

        self.mipmap_gen_pipeline.with_configuration(
            device,
            formats,
            blend,
            write_mask,
            depth_stencil,
            multiview_mask,
        )
    }

    pub fn part_pipeline_with_configuration(
        &mut self,
        device: &wgpu::Device,
        formats: <basic_frag::Shader as FragmentShader>::TargetArray<Option<wgpu::TextureFormat>>,
        blend: <basic_frag::Shader as FragmentShader>::TargetArray<Option<wgpu::BlendState>>,
        write_mask: <basic_frag::Shader as FragmentShader>::TargetArray<wgpu::ColorWrites>,
        depth_stencil: Option<wgpu::DepthStencilState>,
        multiview_mask: Option<NonZero<u32>>,
    ) -> pipeline::Pipeline<basic_vert::Shader, basic_frag::Shader> {
        if let Some(tether) = self.tether {
            if let Some(premade_entry) = tether.part_pipeline.with_configuration_cached(
                formats,
                blend,
                write_mask,
                depth_stencil.clone(),
                multiview_mask,
            ) {
                return premade_entry;
            }
        }
        self.part_pipeline.with_configuration(
            device,
            formats,
            blend,
            write_mask,
            depth_stencil,
            multiview_mask,
        )
    }

    pub fn part_mask_pipeline_with_configuration(
        &mut self,
        device: &wgpu::Device,
        formats: <basic_mask_frag::Shader as FragmentShader>::TargetArray<
            Option<wgpu::TextureFormat>,
        >,
        blend: <basic_mask_frag::Shader as FragmentShader>::TargetArray<Option<wgpu::BlendState>>,
        write_mask: <basic_mask_frag::Shader as FragmentShader>::TargetArray<wgpu::ColorWrites>,
        depth_stencil: Option<wgpu::DepthStencilState>,
        multiview_mask: Option<NonZero<u32>>,
    ) -> pipeline::Pipeline<basic_vert::Shader, basic_mask_frag::Shader> {
        if let Some(tether) = self.tether {
            if let Some(premade_entry) = tether.part_mask_pipeline.with_configuration_cached(
                formats,
                blend,
                write_mask,
                depth_stencil.clone(),
                multiview_mask,
            ) {
                return premade_entry;
            }
        }
        self.part_mask_pipeline.with_configuration(
            device,
            formats,
            blend,
            write_mask,
            depth_stencil,
            multiview_mask,
        )
    }

    pub fn composite_pipeline_with_configuration(
        &mut self,
        device: &wgpu::Device,
        formats: <composite_frag::Shader as FragmentShader>::TargetArray<
            Option<wgpu::TextureFormat>,
        >,
        blend: <composite_frag::Shader as FragmentShader>::TargetArray<Option<wgpu::BlendState>>,
        write_mask: <composite_frag::Shader as FragmentShader>::TargetArray<wgpu::ColorWrites>,
        depth_stencil: Option<wgpu::DepthStencilState>,
        multiview_mask: Option<NonZero<u32>>,
    ) -> pipeline::Pipeline<composite_vert::Shader, composite_frag::Shader> {
        if let Some(tether) = self.tether {
            if let Some(premade_entry) = tether.composite_pipeline.with_configuration_cached(
                formats,
                blend,
                write_mask,
                depth_stencil.clone(),
                multiview_mask,
            ) {
                return premade_entry;
            }
        }

        self.composite_pipeline.with_configuration(
            device,
            formats,
            blend,
            write_mask,
            depth_stencil,
            multiview_mask,
        )
    }

    /// Merge in another pipeline cache into this one.
    ///
    /// This allows multithreaded access to the pipeline cache to account for
    /// new objects that were created during processing.
    pub fn merge(&mut self, other: PipelineCache<'_>) {
        self.part_pipeline.merge(other.part_pipeline);
        self.part_mask_pipeline.merge(other.part_mask_pipeline);
        self.composite_pipeline.merge(other.composite_pipeline);
        self.mipmap_gen_pipeline.merge(other.mipmap_gen_pipeline);
        self.clear_pipeline.merge(other.clear_pipeline);
    }

    /// Determine how many pipelines are cached in this pipeline cache.
    pub fn len(&self) -> usize {
        self.part_pipeline.len()
            + self.part_mask_pipeline.len()
            + self.composite_pipeline.len()
            + self.mipmap_gen_pipeline.len()
            + self.clear_pipeline.len()
    }
}
