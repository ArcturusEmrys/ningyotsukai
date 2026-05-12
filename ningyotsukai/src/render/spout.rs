use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;
use windows::Win32::Graphics::Dxgi::IDXGIResource;
use windows::core::Interface;

use ningyo_spout2::{Registration, SenderRegistry};
use ningyo_texshare::{ExportableTexture, ExtendedDevice};

use crate::render::SinkPlugin;

pub struct SpoutPlugin {
    sender_registry: SenderRegistry,
    registration: Option<Registration>,
    internal_texture: Option<ExportableTexture>,
}

impl SpoutPlugin {
    pub fn new() -> Box<dyn SinkPlugin> {
        Box::new(SpoutPlugin {
            sender_registry: SenderRegistry::new().unwrap(),
            registration: None,
            internal_texture: None,
        }) as Box<dyn SinkPlugin>
    }
}

impl SinkPlugin for SpoutPlugin {
    fn publish_stream(
        &mut self,
        _document: crate::document::Document,
        name: String,
        _size: glam::Vec2,
        _framerate: (u32, u32),
    ) {
        let registration = self.sender_registry.register(&name).unwrap();
        self.registration = Some(registration);
    }

    fn update_stream_image(
        &mut self,
        document: crate::document::Document,
        adapter: &wgpu::Adapter,
        device: &ExtendedDevice,
        queue: &wgpu::Queue,
        texture: wgpu::Texture,
    ) {
        // TODO: Detect a texture size change.
        if self.internal_texture.is_none()
            || self
                .internal_texture
                .as_ref()
                .map(|t| {
                    t.texture().width() != texture.width()
                        || t.texture().height() != texture.height()
                })
                .unwrap_or(false)
        {
            self.internal_texture = device.create_texture_exportable(
                adapter,
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("Internal Spout2 Buffer"),
                    size: wgpu::Extent3d {
                        width: texture.width() as u32,
                        height: texture.height() as u32,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                },
            );
        }

        let internal_texture = self.internal_texture.clone().unwrap();
        let share_handle = internal_texture
            .as_d3d_resource(device.device())
            .unwrap()
            .as_share_handle(device)
            .unwrap();

        let mut encoder = device
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Internal Spout2 Buffer Copy"),
            });

        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &internal_texture.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: texture.width(),
                height: texture.height(),
                depth_or_array_layers: 0,
            },
        );

        if let Some(reg) = &mut self.registration {
            reg.publish_dx11_texture(
                texture.width(),
                texture.height(),
                DXGI_FORMAT_R8G8B8A8_UNORM.0 as u32,
                share_handle,
            )
            .unwrap();
        }
    }
}
