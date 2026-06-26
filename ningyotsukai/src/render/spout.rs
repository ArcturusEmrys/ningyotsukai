use windows::Win32::Graphics::Direct3D11::{ID3D11RenderTargetView, ID3D11Resource};
use windows::Win32::Graphics::Dxgi::IDXGIResource;
use windows::Win32::Graphics::{
    Direct3D11::ID3D11Texture2D, Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM,
};
use windows::core::Interface;

use ningyo_spout2::{Registration, SenderRegistry};
use ningyo_texshare::{ExportableTexture, ExtendedDevice};

use crate::render::SinkPlugin;

pub struct SpoutPlugin {
    sender_registry: SenderRegistry,
    registration: Option<Registration>,
    d3d12_buffer_texture: Option<ExportableTexture>,
    d3d12_wrapped_resource: Option<ID3D11Resource>,
    internal_dx11_texture: Option<ID3D11Texture2D>,
    internal_render_view: Option<ID3D11RenderTargetView>,
}

impl SpoutPlugin {
    pub fn new() -> Box<dyn SinkPlugin> {
        Box::new(SpoutPlugin {
            sender_registry: SenderRegistry::new().unwrap(),
            registration: None,
            d3d12_buffer_texture: None,
            d3d12_wrapped_resource: None,
            internal_dx11_texture: None,
            internal_render_view: None,
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

    fn stream_parameters_changed(
        &mut self,
        _document: crate::document::Document,
        _name: String,
        _size: glam::Vec2,
        _framerate: (u32, u32),
    ) {
        // Spout2 does not care about the size of the texture, so we don't need
        // to do anything here. The video size can change every frame for all
        // we care.
    }

    fn update_stream_image(
        &mut self,
        _document: crate::document::Document,
        adapter: &wgpu::Adapter,
        device: &ExtendedDevice,
        queue: &wgpu::Queue,
        texture: wgpu::Texture,
        origin: wgpu::Origin3d,
        extent: wgpu::Extent3d,
    ) {
        // TODO: Detect a texture size change.
        if self.d3d12_buffer_texture.is_none()
            || self
                .d3d12_buffer_texture
                .as_ref()
                .map(|t| {
                    t.texture().width() != extent.width || t.texture().height() != extent.height
                })
                .unwrap_or(false)
        {
            let desc = wgpu::TextureDescriptor {
                label: Some("Internal Spout2 Buffer"),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: texture.format(),
                usage: wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            };
            self.d3d12_buffer_texture = device.create_texture_exportable(adapter, queue, &desc);
            self.d3d12_wrapped_resource = Some(
                self.d3d12_buffer_texture
                    .as_ref()
                    .unwrap()
                    .as_d3d_resource()
                    .unwrap()
                    .into_d3d11_resource(device, queue)
                    .unwrap(),
            );

            self.internal_dx11_texture = device.create_dx11_texture(&desc);
            unsafe {
                device
                    .d3d11_device()
                    .CreateRenderTargetView(
                        self.internal_dx11_texture.as_ref().unwrap(),
                        None,
                        Some(&mut self.internal_render_view),
                    )
                    .unwrap();
            }
        }

        // Since we're rendering to an array texture now, we HAVE to first do
        // an extraction copy into a 2D texture BEFORE we can have D3D11 copy
        // its texture into one we can safely send through Spout.
        let internal_texture = self.d3d12_buffer_texture.clone().unwrap();
        let mut encoder = device
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("First Spout2 Buffer Copy"),
            });

        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &internal_texture.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            extent,
        );

        let submission_index = queue.submit(std::iter::once(encoder.finish()));

        device
            .device()
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission_index),
                timeout: None,
            })
            .unwrap();

        // Now D3D11 has to do its own copy :/
        let wrapped_d3d11_texture = self.d3d12_wrapped_resource.clone().unwrap();
        let internal_dx11_texture = self.internal_dx11_texture.clone().unwrap();

        unsafe {
            device
                .d3d11on12_device()
                .AcquireWrappedResources(&[Some(wrapped_d3d11_texture.clone())]);
            device.d3d11_context().CopyResource(
                &internal_dx11_texture.cast::<ID3D11Resource>().unwrap(),
                &wrapped_d3d11_texture,
            );
            device
                .d3d11on12_device()
                .ReleaseWrappedResources(&[Some(wrapped_d3d11_texture.clone())]);
            device.d3d11_context().Flush();
        }

        let dxgi_texture: IDXGIResource = internal_dx11_texture.cast().unwrap();
        let share_handle = unsafe { dxgi_texture.GetSharedHandle().unwrap() };

        if let Some(reg) = &mut self.registration {
            reg.publish_dx11_texture(
                extent.width,
                extent.height,
                //TODO: What if GTK wants a different format?
                DXGI_FORMAT_R8G8B8A8_UNORM.0 as u32,
                share_handle,
            )
            .unwrap();
        }
    }
}
