//! Shared resource management.
//!
//! This type enables having multiple renderers share resources such as shaders,
//! and pipelines.

use std::sync::RwLock;

use glam::Vec2;
use wgpu;
#[cfg(feature = "tracy")]
use wgpu::CommandEncoder;
use wgpu::util::DeviceExt;

use crate::error::WgpuRendererError;
use crate::pipeline;
use crate::shader::FragmentShader;
use crate::shaders::basic::{
    basic_frag, basic_mask_frag, basic_vert, composite_frag, composite_vert,
};
use crate::shaders::{mipmap_gen_frag, mipmap_gen_vert, null_frag};
use crate::uploads::cast_vec2;

/// WGPU resources that are invariant to the current puppet being rendered.
///
/// Multiple renderes may share resources so long as they use the same device
/// and queue.
///
/// It is recommended to shove this in an Arc<Mutex<>> so it can be shared
/// across all renderers in a process.
pub struct WgpuResources {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,

    pub(crate) model_sampler: wgpu::Sampler,

    pub(crate) mipmap_gen_vert_bind: wgpu::BindGroup,
    pub(crate) mipmap_gen_frag: mipmap_gen_frag::Shader,
    pub(crate) mipmap_gen_triangles: wgpu::Buffer,

    pub(crate) null_frag_bind: wgpu::BindGroup,

    pub(crate) part_shader_vert: basic_vert::Shader,
    pub(crate) part_shader_frag: basic_frag::Shader,
    pub(crate) part_shader_mask_frag: basic_mask_frag::Shader,

    pub(crate) masked_depthstencil: wgpu::DepthStencilState,
    pub(crate) mask_depthstencil: wgpu::DepthStencilState,
    pub(crate) clear_depthstencil: wgpu::DepthStencilState,
    pub(crate) ignore_depthstencil: wgpu::DepthStencilState,

    pub(crate) composite_shader_vert_bind: wgpu::BindGroup,
    pub(crate) composite_shader_frag: composite_frag::Shader,

    pipelines: RwLock<WgpuResourcesMutable>,
}

struct WgpuResourcesMutable {
    pub(crate) clear_pipeline: pipeline::PipelineGroup<mipmap_gen_vert::Shader, null_frag::Shader>,
    pub(crate) mipmap_gen_pipeline:
        pipeline::PipelineGroup<mipmap_gen_vert::Shader, mipmap_gen_frag::Shader>,
    pub(crate) part_pipeline: pipeline::PipelineGroup<basic_vert::Shader, basic_frag::Shader>,
    pub(crate) part_mask_pipeline:
        pipeline::PipelineGroup<basic_vert::Shader, basic_mask_frag::Shader>,
    pub(crate) composite_pipeline:
        pipeline::PipelineGroup<composite_vert::Shader, composite_frag::Shader>,

    #[cfg(feature = "tracy")]
    pub profiler: wgpu_profiler::GpuProfiler,
}

impl WgpuResources {
    /// Retrieve the rendering library's preferred set of device capabilities.
    ///
    /// Externally-created devices must have, at minimum, all of the features
    /// listed in this device descriptor.
    pub fn preferred_device_descriptor() -> wgpu::DeviceDescriptor<'static> {
        #[allow(unused_mut)]
        let mut dd = wgpu::DeviceDescriptor {
            required_features: wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER
                | wgpu::Features::CLEAR_TEXTURE
                | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                | wgpu::Features::DEPTH_CLIP_CONTROL,
            required_limits: wgpu::Limits {
                max_color_attachment_bytes_per_sample: 48,
                ..Default::default()
            },
            ..Default::default()
        };

        #[cfg(feature = "tracy")]
        {
            dd.required_features |= wgpu_profiler::GpuProfiler::ALL_WGPU_TIMER_FEATURES;
        }

        dd
    }

    /// Obtain a device and queue with the given adapter and load our resources
    /// into it.
    pub async fn new(adapter: &wgpu::Adapter) -> Result<Self, WgpuRendererError> {
        let (device, queue) = adapter
            .request_device(&Self::preferred_device_descriptor())
            .await?;

        Ok(Self::new_with_user_device(device, queue))
    }

    /// Load all resources into the given device and queue.
    ///
    /// You must ensure that the given device and queue were acquired using the
    /// `preferred_device_descriptor` or a superset of its capabilities.
    pub fn new_with_user_device(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        #[cfg(feature = "tracy")]
        let profiler = wgpu_profiler::GpuProfiler::new_with_tracy_client(
            wgpu_profiler::GpuProfilerSettings::default(),
            device.adapter_info().backend,
            &device,
            &queue,
        )
        .unwrap();

        // Compile all our shaders now.
        let part_shader_vert = basic_vert::Shader::new(&device, true);
        let part_shader_frag = basic_frag::Shader::new(&device, true);
        let part_shader_mask_frag = basic_mask_frag::Shader::new(&device, true);

        let masked_depthstencil = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState {
                front: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Equal,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                back: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Equal,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                read_mask: 0xFF,
                write_mask: 0x00,
            },
            bias: wgpu::DepthBiasState::default(),
        };

        let mask_depthstencil = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState {
                front: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                back: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Keep,
                    depth_fail_op: wgpu::StencilOperation::Keep,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                read_mask: 0xFF,
                write_mask: 0xFF,
            },
            bias: wgpu::DepthBiasState::default(),
        };

        let clear_depthstencil = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState {
                front: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Replace,
                    depth_fail_op: wgpu::StencilOperation::Replace,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                back: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Replace,
                    depth_fail_op: wgpu::StencilOperation::Replace,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                read_mask: 0xFF,
                write_mask: 0xFF,
            },
            bias: wgpu::DepthBiasState::default(),
        };

        let ignore_depthstencil = wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState {
                front: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Replace,
                    depth_fail_op: wgpu::StencilOperation::Replace,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                back: wgpu::StencilFaceState {
                    compare: wgpu::CompareFunction::Always,
                    fail_op: wgpu::StencilOperation::Replace,
                    depth_fail_op: wgpu::StencilOperation::Replace,
                    pass_op: wgpu::StencilOperation::Replace,
                },
                read_mask: 0x00,
                write_mask: 0x00,
            },
            bias: wgpu::DepthBiasState::default(),
        };

        //TODO: We need a pipeline per Inochi blending mode
        //(or some kind of ubershader blending)

        let part_pipeline =
            pipeline::PipelineGroup::new(part_shader_vert.clone(), part_shader_frag.clone());
        let part_mask_pipeline =
            pipeline::PipelineGroup::new(part_shader_vert.clone(), part_shader_mask_frag.clone());

        let composite_shader_vert = composite_vert::Shader::new(&device);
        let composite_shader_vert_bind = composite_shader_vert.bind(&device);
        let composite_shader_frag = composite_frag::Shader::new(&device, true);

        let composite_pipeline = pipeline::PipelineGroup::new(
            composite_shader_vert.clone(),
            composite_shader_frag.clone(),
        );

        let mipmap_gen_vert = mipmap_gen_vert::Shader::new(&device);
        let mipmap_gen_vert_bind = mipmap_gen_vert.bind(&device);
        let mipmap_gen_frag = mipmap_gen_frag::Shader::new(&device);
        let mipmap_gen_triangles = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mipmap Generator Quad"),
            usage: wgpu::BufferUsages::VERTEX,
            contents: cast_vec2(&[
                Vec2::new(-1.0, -1.0),
                Vec2::new(1.0, -1.0),
                Vec2::new(-1.0, 1.0),
                Vec2::new(-1.0, 1.0),
                Vec2::new(1.0, -1.0),
                Vec2::new(1.0, 1.0),
            ]),
        });
        let mipmap_gen_pipeline =
            pipeline::PipelineGroup::new(mipmap_gen_vert.clone(), mipmap_gen_frag.clone());

        let model_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToBorder,
            address_mode_v: wgpu::AddressMode::ClampToBorder,
            address_mode_w: wgpu::AddressMode::ClampToBorder,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });

        let null_frag = null_frag::Shader::new(&device);
        let null_frag_bind = null_frag.bind(&device);
        let clear_pipeline =
            pipeline::PipelineGroup::new(mipmap_gen_vert.clone(), null_frag.clone());

        // Flush all pending work.
        // In wgpu, texture uploads etc will only execute at submit time
        queue.submit([]);

        WgpuResources {
            device,
            queue,

            model_sampler,
            mipmap_gen_vert_bind,
            mipmap_gen_frag,
            mipmap_gen_triangles,
            part_shader_vert,
            part_shader_frag,
            part_shader_mask_frag,
            mask_depthstencil,
            masked_depthstencil,
            clear_depthstencil,
            ignore_depthstencil,
            composite_shader_vert_bind,
            composite_shader_frag,
            null_frag_bind,
            pipelines: RwLock::new(WgpuResourcesMutable {
                #[cfg(feature = "tracy")]
                profiler,

                clear_pipeline,
                mipmap_gen_pipeline,
                part_pipeline,
                part_mask_pipeline,
                composite_pipeline,
            }),
        }
    }

    pub fn clear_pipeline_with_configuration(
        &self,
        device: &wgpu::Device,
        formats: <null_frag::Shader as FragmentShader>::TargetArray<Option<wgpu::TextureFormat>>,
        blend: <null_frag::Shader as FragmentShader>::TargetArray<Option<wgpu::BlendState>>,
        write_mask: <null_frag::Shader as FragmentShader>::TargetArray<wgpu::ColorWrites>,
        depth_stencil: Option<wgpu::DepthStencilState>,
    ) -> pipeline::Pipeline<mipmap_gen_vert::Shader, null_frag::Shader> {
        if let Some(premade_entry) = self
            .pipelines
            .read()
            .unwrap()
            .clear_pipeline
            .with_configuration_cached(formats, blend, write_mask, depth_stencil.clone())
        {
            premade_entry
        } else {
            self.pipelines
                .write()
                .unwrap()
                .clear_pipeline
                .with_configuration(device, formats, blend, write_mask, depth_stencil)
        }
    }

    pub fn mipmap_gen_pipeline_with_configuration(
        &self,
        device: &wgpu::Device,
        formats: <mipmap_gen_frag::Shader as FragmentShader>::TargetArray<
            Option<wgpu::TextureFormat>,
        >,
        blend: <mipmap_gen_frag::Shader as FragmentShader>::TargetArray<Option<wgpu::BlendState>>,
        write_mask: <mipmap_gen_frag::Shader as FragmentShader>::TargetArray<wgpu::ColorWrites>,
        depth_stencil: Option<wgpu::DepthStencilState>,
    ) -> pipeline::Pipeline<mipmap_gen_vert::Shader, mipmap_gen_frag::Shader> {
        if let Some(premade_entry) = self
            .pipelines
            .read()
            .unwrap()
            .mipmap_gen_pipeline
            .with_configuration_cached(formats, blend, write_mask, depth_stencil.clone())
        {
            premade_entry
        } else {
            self.pipelines
                .write()
                .unwrap()
                .mipmap_gen_pipeline
                .with_configuration(device, formats, blend, write_mask, depth_stencil)
        }
    }

    pub fn part_pipeline_with_configuration(
        &self,
        device: &wgpu::Device,
        formats: <basic_frag::Shader as FragmentShader>::TargetArray<Option<wgpu::TextureFormat>>,
        blend: <basic_frag::Shader as FragmentShader>::TargetArray<Option<wgpu::BlendState>>,
        write_mask: <basic_frag::Shader as FragmentShader>::TargetArray<wgpu::ColorWrites>,
        depth_stencil: Option<wgpu::DepthStencilState>,
    ) -> pipeline::Pipeline<basic_vert::Shader, basic_frag::Shader> {
        if let Some(premade_entry) = self
            .pipelines
            .read()
            .unwrap()
            .part_pipeline
            .with_configuration_cached(formats, blend, write_mask, depth_stencil.clone())
        {
            premade_entry
        } else {
            self.pipelines
                .write()
                .unwrap()
                .part_pipeline
                .with_configuration(device, formats, blend, write_mask, depth_stencil)
        }
    }

    pub fn part_mask_pipeline_with_configuration(
        &self,
        device: &wgpu::Device,
        formats: <basic_mask_frag::Shader as FragmentShader>::TargetArray<
            Option<wgpu::TextureFormat>,
        >,
        blend: <basic_mask_frag::Shader as FragmentShader>::TargetArray<Option<wgpu::BlendState>>,
        write_mask: <basic_mask_frag::Shader as FragmentShader>::TargetArray<wgpu::ColorWrites>,
        depth_stencil: Option<wgpu::DepthStencilState>,
    ) -> pipeline::Pipeline<basic_vert::Shader, basic_mask_frag::Shader> {
        if let Some(premade_entry) = self
            .pipelines
            .read()
            .unwrap()
            .part_mask_pipeline
            .with_configuration_cached(formats, blend, write_mask, depth_stencil.clone())
        {
            premade_entry
        } else {
            self.pipelines
                .write()
                .unwrap()
                .part_mask_pipeline
                .with_configuration(device, formats, blend, write_mask, depth_stencil)
        }
    }

    pub fn composite_pipeline_with_configuration(
        &self,
        device: &wgpu::Device,
        formats: <composite_frag::Shader as FragmentShader>::TargetArray<
            Option<wgpu::TextureFormat>,
        >,
        blend: <composite_frag::Shader as FragmentShader>::TargetArray<Option<wgpu::BlendState>>,
        write_mask: <composite_frag::Shader as FragmentShader>::TargetArray<wgpu::ColorWrites>,
        depth_stencil: Option<wgpu::DepthStencilState>,
    ) -> pipeline::Pipeline<composite_vert::Shader, composite_frag::Shader> {
        if let Some(premade_entry) = self
            .pipelines
            .read()
            .unwrap()
            .composite_pipeline
            .with_configuration_cached(formats, blend, write_mask, depth_stencil.clone())
        {
            premade_entry
        } else {
            self.pipelines
                .write()
                .unwrap()
                .composite_pipeline
                .with_configuration(device, formats, blend, write_mask, depth_stencil)
        }
    }

    #[cfg(feature = "tracy")]
    pub fn start_query(&self, encoder: &mut CommandEncoder) -> wgpu_profiler::GpuProfilerQuery {
        self.pipelines
            .write()
            .unwrap()
            .profiler
            .begin_query("WgpuDrawSession::begin", encoder)
    }

    #[cfg(feature = "tracy")]
    pub fn end_query(
        &self,
        encoder: &mut CommandEncoder,
        encoder_query: wgpu_profiler::GpuProfilerQuery,
    ) {
        self.pipelines
            .write()
            .unwrap()
            .profiler
            .end_query(encoder, encoder_query);
    }

    /// End the current profiler frame.
    ///
    /// This is responsible for cleaning up the profiler frame and is intended
    /// to be called after all renderers for the frame have run.
    #[cfg(feature = "tracy")]
    pub fn end_frame(&self) -> Result<(), wgpu_profiler::EndFrameError> {
        let mut mutable_bit = self.pipelines.write().unwrap();
        let profiler = &mut mutable_bit.profiler;
        let mut command_encoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("WgpuResources::end_frame"),
                });

        profiler.resolve_queries(&mut command_encoder);
        self.queue.submit(std::iter::once(command_encoder.finish()));

        profiler.end_frame()?;

        let _ = profiler.process_finished_frame(self.queue.get_timestamp_period());

        Ok(())
    }
}
