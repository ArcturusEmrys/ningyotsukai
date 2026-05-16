//! Shared resource management.
//!
//! This type enables having multiple renderers share resources such as shaders,
//! and pipelines.

use glam::Vec2;
use wgpu;
use wgpu::util::DeviceExt;

use crate::error::WgpuRendererError;
use crate::pipeline;
use crate::shaders::basic::{
    basic_frag, basic_mask_frag, basic_vert, composite_frag, composite_mask_frag, composite_vert,
};
use crate::shaders::{mipmap_gen_frag, mipmap_gen_vert};
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

    pub(crate) mipmap_gen_vert: mipmap_gen_vert::Shader,
    pub(crate) mipmap_gen_frag: mipmap_gen_frag::Shader,
    pub(crate) mipmap_gen_triangles: wgpu::Buffer,
    pub(crate) mipmap_gen_pipeline:
        pipeline::PipelineGroup<mipmap_gen_vert::Shader, mipmap_gen_frag::Shader>,

    pub(crate) part_shader_vert: basic_vert::Shader,
    pub(crate) part_shader_frag: basic_frag::Shader,
    pub(crate) part_shader_mask_frag: basic_mask_frag::Shader,

    pub(crate) masked_depthstencil: wgpu::DepthStencilState,
    pub(crate) mask_depthstencil: wgpu::DepthStencilState,

    pub(crate) part_pipeline: pipeline::PipelineGroup<basic_vert::Shader, basic_frag::Shader>,
    pub(crate) part_mask_pipeline:
        pipeline::PipelineGroup<basic_vert::Shader, basic_mask_frag::Shader>,

    pub(crate) composite_shader_vert: composite_vert::Shader,
    pub(crate) composite_shader_frag: composite_frag::Shader,
    pub(crate) _composite_shader_mask_frag: composite_mask_frag::Shader,

    pub(crate) composite_pipeline:
        pipeline::PipelineGroup<composite_vert::Shader, composite_frag::Shader>,
    pub(crate) _composite_mask_pipeline:
        pipeline::PipelineGroup<composite_vert::Shader, composite_mask_frag::Shader>,
}

impl WgpuResources {
    /// Retrieve the rendering library's preferred set of device capabilities.
    ///
    /// Externally-created devices must have, at minimum, all of the features
    /// listed in this device descriptor.
    pub fn preferred_device_descriptor() -> wgpu::DeviceDescriptor<'static> {
        wgpu::DeviceDescriptor {
            required_features: wgpu::Features::ADDRESS_MODE_CLAMP_TO_BORDER
                | wgpu::Features::CLEAR_TEXTURE
                | wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                | wgpu::Features::DEPTH_CLIP_CONTROL,
            required_limits: wgpu::Limits {
                max_color_attachment_bytes_per_sample: 48,
                ..Default::default()
            },
            ..Default::default()
        }
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
        // Compile all our shaders now.
        let part_shader_vert = basic_vert::Shader::new(&device);
        let part_shader_frag = basic_frag::Shader::new(&device);
        let part_shader_mask_frag = basic_mask_frag::Shader::new(&device);

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

        //TODO: We need a pipeline per Inochi blending mode
        //(or some kind of ubershader blending)

        let part_pipeline =
            pipeline::PipelineGroup::new(part_shader_vert.clone(), part_shader_frag.clone());
        let part_mask_pipeline =
            pipeline::PipelineGroup::new(part_shader_vert.clone(), part_shader_mask_frag.clone());

        let composite_shader_vert = composite_vert::Shader::new(&device);
        let composite_shader_frag = composite_frag::Shader::new(&device);
        let composite_shader_mask_frag = composite_mask_frag::Shader::new(&device);

        let composite_pipeline = pipeline::PipelineGroup::new(
            composite_shader_vert.clone(),
            composite_shader_frag.clone(),
        );
        let composite_mask_pipeline = pipeline::PipelineGroup::new(
            composite_shader_vert.clone(),
            composite_shader_mask_frag.clone(),
        );

        let mipmap_gen_vert = mipmap_gen_vert::Shader::new(&device);
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

        // Flush all pending work.
        // In wgpu, texture uploads etc will only execute at submit time
        queue.submit([]);

        WgpuResources {
            device,
            queue,
            model_sampler,
            mipmap_gen_vert,
            mipmap_gen_frag,
            mipmap_gen_triangles,
            mipmap_gen_pipeline,
            part_shader_vert,
            part_shader_frag,
            part_shader_mask_frag,
            mask_depthstencil,
            masked_depthstencil,
            part_pipeline,
            part_mask_pipeline,
            composite_shader_vert,
            composite_shader_frag,
            _composite_shader_mask_frag: composite_shader_mask_frag,
            composite_pipeline,
            _composite_mask_pipeline: composite_mask_pipeline,
        }
    }
}
