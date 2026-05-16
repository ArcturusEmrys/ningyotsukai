use std::cmp::min;

use wgpu;

use inox2d::model::Model;
use inox2d::texture::ShallowTexture;

use crate::WgpuResources;
use crate::error::WgpuRendererError;
use crate::shaders::mipmap_gen_vert;

#[derive(Clone)]
pub struct DeviceTexture {
    device_texture: wgpu::Texture,
    view: wgpu::TextureView,
}

impl DeviceTexture {
    /// Submit a texture to be uploaded to the given WGPU device.
    ///
    /// Note that the upload will not complete until the next queue submission.
    pub fn new_from_model(
        resources: &mut WgpuResources,
        model: &Model,
        index: usize,
        texture: &ShallowTexture,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: texture.width(),
            height: texture.height(),
            depth_or_array_layers: 1,
        };
        let mip_level_count = (min(texture.width(), texture.height()) as f64)
            .log2()
            .floor() as u32;
        let device_texture = resources.device.create_texture(&wgpu::TextureDescriptor {
            size,
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            label: Some(&format!(
                "Puppet texture: {}::{}",
                model
                    .puppet
                    .meta
                    .name
                    .as_deref()
                    .unwrap_or("<NAME NOT PROVIDED>"),
                index
            )),
            view_formats: &[],
        });

        resources.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &device_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            texture.pixels(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * texture.width()),
                rows_per_image: Some(texture.height()),
            },
            size,
        );

        let mut encoder =
            resources
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("DeviceTexture::new_from_model - internal mipmap generation"),
                });

        let mut input_view = device_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("DeviceTexture::new_from_model - mipmap target view 0"),
            format: None,
            dimension: None,
            usage: None,
            aspect: wgpu::TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: Some(1),
            base_array_layer: 0,
            array_layer_count: None,
        });

        for level in 1..mip_level_count {
            let output_view = device_texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!(
                    "DeviceTexture::new_from_model - mipmap target view {}",
                    level
                )),
                format: None,
                dimension: None,
                usage: None,
                aspect: wgpu::TextureAspect::All,
                base_mip_level: level,
                mip_level_count: Some(1),
                base_array_layer: 0,
                array_layer_count: None,
            });

            let input_sampler = resources.device.create_sampler(&wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToBorder,
                address_mode_v: wgpu::AddressMode::ClampToBorder,
                address_mode_w: wgpu::AddressMode::ClampToBorder,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                lod_max_clamp: (level - 1) as f32,
                ..Default::default()
            });

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(&format!(
                    "DeviceTexture::new_from_model - mipmap scale pass for level {}",
                    level
                )),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &output_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::DontCare(wgpu::LoadOpDontCare::default()),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            let pipeline = resources.mipmap_gen_pipeline.with_configuration(
                &resources.device,
                [Some(device_texture.format())],
                [Some(wgpu::BlendState::REPLACE)],
                [wgpu::ColorWrites::all()],
                None,
            );

            render_pass.set_vertex_buffer(
                mipmap_gen_vert::INPUT_LOCATION_VERTS,
                resources.mipmap_gen_triangles.slice(..),
            );
            render_pass.set_pipeline(pipeline.pipeline());
            pipeline.bind_frag(
                &mut render_pass,
                Some(&resources.mipmap_gen_frag.bind(
                    &resources.device,
                    &input_view,
                    &input_sampler,
                )),
            );
            pipeline.bind_vertex(
                &mut render_pass,
                Some(&resources.mipmap_gen_vert.bind(&resources.device)),
            );
            render_pass.draw(0..6, 0..1);

            input_view = output_view;
        }

        resources.queue.submit(std::iter::once(encoder.finish()));

        let view = device_texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            device_texture,
            view,
        }
    }

    pub fn required_render_target_uses() -> wgpu::TextureUsages {
        wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_DST
    }

    pub fn empty_render_target(
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let device_texture = device.create_texture(&wgpu::TextureDescriptor {
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: Self::required_render_target_uses(),
            label: Some("GBuffer"),
            view_formats: &[],
        });

        let view = device_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let empty = Self {
            device_texture,
            view,
        };

        empty.clear(encoder);
        empty
    }

    /// Adopt a user-provided texture for use as a render target.
    ///
    /// The texture must have a superset of the usages prescribed by
    /// `required_render_target_uses`.
    pub fn user_render_target(
        device_texture: wgpu::Texture,
    ) -> Result<DeviceTexture, WgpuRendererError> {
        if !device_texture
            .usage()
            .contains(Self::required_render_target_uses())
        {
            return Err(WgpuRendererError::InvalidRenderTargetTexture);
        }

        let view = device_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let empty = Self {
            device_texture,
            view,
        };

        Ok(empty)
    }

    // Clear the texture.
    pub fn clear(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.clear_texture(
            self.texture(),
            &wgpu::ImageSubresourceRange {
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            },
        );
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.device_texture
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn as_color_attachment(&self) -> wgpu::RenderPassColorAttachment<'_> {
        wgpu::RenderPassColorAttachment {
            view: &self.view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        }
    }
}

pub struct DepthStencilTexture {
    device_texture: wgpu::Texture,
    view: wgpu::TextureView,
    format: wgpu::TextureFormat,
}

impl DepthStencilTexture {
    pub fn empty_render_target(
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
    ) -> Self {
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let device_texture = device.create_texture(&wgpu::TextureDescriptor {
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST,
            label: Some("GBuffer"),
            view_formats: &[],
        });

        let view = device_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let empty = Self {
            device_texture,
            view,
            format,
        };

        empty.clear(encoder);
        empty
    }

    // Clear the texture.
    pub fn clear(&self, encoder: &mut wgpu::CommandEncoder) {
        encoder.clear_texture(
            self.texture(),
            &wgpu::ImageSubresourceRange {
                aspect: wgpu::TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            },
        );
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.device_texture
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn as_depth_stencil_attachment_rw(&self) -> wgpu::RenderPassDepthStencilAttachment<'_> {
        wgpu::RenderPassDepthStencilAttachment {
            view: &self.view,
            depth_ops: None,
            stencil_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            }),
        }
    }

    pub fn as_depth_stencil_attachment_ro(&self) -> wgpu::RenderPassDepthStencilAttachment<'_> {
        wgpu::RenderPassDepthStencilAttachment {
            view: &self.view,
            depth_ops: None,
            stencil_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Discard,
            }),
        }
    }

    pub fn as_depth_stencil_attachment_clear(
        &self,
        clear_value: u32,
    ) -> wgpu::RenderPassDepthStencilAttachment<'_> {
        wgpu::RenderPassDepthStencilAttachment {
            view: &self.view,
            depth_ops: None,
            stencil_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(clear_value),
                store: wgpu::StoreOp::Store,
            }),
        }
    }
}

/// Structure that holds render targets for interim rendering results.
pub struct GBuffer {
    albedo: DeviceTexture,
    emissive: DeviceTexture,
    bump: DeviceTexture,
    stencil: DepthStencilTexture,
}

impl GBuffer {
    pub fn new(
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        Self {
            albedo: DeviceTexture::empty_render_target(
                device,
                encoder,
                width,
                height,
                wgpu::TextureFormat::Rgba8Unorm,
            ),
            emissive: DeviceTexture::empty_render_target(device, encoder, width, height, format),
            bump: DeviceTexture::empty_render_target(
                device,
                encoder,
                width,
                height,
                wgpu::TextureFormat::Rgba8Unorm,
            ),
            stencil: DepthStencilTexture::empty_render_target(
                device,
                encoder,
                width,
                height,
                depth_format,
            ),
        }
    }

    pub fn albedo(&self) -> &DeviceTexture {
        &self.albedo
    }

    pub fn emissive(&self) -> &DeviceTexture {
        &self.emissive
    }

    pub fn bump(&self) -> &DeviceTexture {
        &self.bump
    }

    pub fn stencil(&self) -> &DepthStencilTexture {
        &self.stencil
    }

    pub fn clear(&self, encoder: &mut wgpu::CommandEncoder) {
        self.albedo().clear(encoder);
        self.emissive().clear(encoder);
        self.bump().clear(encoder);
        self.stencil().clear(encoder);
    }

    pub fn as_color_attachments(&self) -> [Option<wgpu::RenderPassColorAttachment<'_>>; 3] {
        [
            Some(self.albedo.as_color_attachment()),
            Some(self.emissive.as_color_attachment()),
            Some(self.bump.as_color_attachment()),
        ]
    }
}
