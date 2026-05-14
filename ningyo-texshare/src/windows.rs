use windows::Win32::Foundation::{GENERIC_ALL, HANDLE};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CREATE_DEVICE_BGRA_SUPPORT,
    D3D11_RESOURCE_MISC_SHARED, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, ID3D11Device,
    ID3D11DeviceContext, ID3D11Resource, ID3D11Texture2D,
};
use windows::Win32::Graphics::Direct3D11on12::{
    D3D11_RESOURCE_FLAGS, D3D11On12CreateDevice, ID3D11On12Device,
};
use windows::Win32::Graphics::Direct3D12::{
    D3D12_RESOURCE_STATE_COPY_SOURCE, D3D12_RESOURCE_STATE_PRESENT, ID3D12Resource,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_SAMPLE_DESC;
use windows::core::Interface;

use wgpu_hal::dx12::Api as Dx12Api;

use crate::dx12::{DeviceExt as Dx12DeviceExt, map_texture_format};
use crate::error::Error as OurError;
use crate::texture::ExportableTexture;
use crate::wgpu::DeviceExt as WgpuDeviceExt;
use crate::wgpu::map_texture_usage_for_texture;

/// An extended device type that can hold a D3D11On12 device for DX11 sharing.
#[derive(Clone)]
pub struct ExtendedDevice {
    inner: wgpu::Device,
    d3d11on12_dev: ID3D11On12Device,
    d3d11_dev: ID3D11Device,
    d3d11_context: ID3D11DeviceContext,
}

impl ExtendedDevice {
    pub fn wrap(device: wgpu::Device) -> Self {
        let dx12_hal = unsafe { device.as_hal::<wgpu_hal::api::Dx12>() }.unwrap();
        let dx12_device = dx12_hal.raw_device();
        let dx12_queue = dx12_hal.raw_queue();

        let mut dx11_device = None;
        let mut dx11_immediate_context = None;

        unsafe {
            D3D11On12CreateDevice(
                dx12_device,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT.0,
                None,
                Some(&[Some(dx12_queue.cast().unwrap())]),
                0,
                Some(&mut dx11_device),
                Some(&mut dx11_immediate_context),
                None,
            )
            .unwrap();
        }

        Self {
            inner: device,
            d3d11on12_dev: dx11_device.clone().unwrap().cast().unwrap(),
            d3d11_dev: dx11_device.unwrap(),
            d3d11_context: dx11_immediate_context.unwrap(),
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.inner
    }

    pub fn d3d11_device(&self) -> ID3D11Device {
        self.d3d11_dev.clone()
    }

    pub fn d3d11_context(&self) -> ID3D11DeviceContext {
        self.d3d11_context.clone()
    }

    pub fn d3d11on12_device(&self) -> ID3D11On12Device {
        self.d3d11on12_dev.clone()
    }

    pub fn create_texture_exportable(
        &self,
        adapter: &wgpu::Adapter,
        queue: &wgpu::Queue,
        texture: &wgpu::TextureDescriptor<'_>,
    ) -> Option<ExportableTexture> {
        if let Some(dx12device) = unsafe { self.inner.as_hal::<Dx12Api>() } {
            let format_features = adapter.get_texture_format_features(texture.format);

            let inner_desc = wgpu_hal::TextureDescriptor {
                label: texture.label.into(),
                size: texture.size,
                mip_level_count: texture.mip_level_count,
                sample_count: texture.sample_count,
                dimension: texture.dimension,
                format: texture.format,
                usage: map_texture_usage_for_texture(texture, &format_features),
                memory_flags: wgpu_hal::MemoryFlags::empty(),
                view_formats: texture.view_formats.to_vec(),
            };

            let (dxtexture, size, alignment) =
                dx12device.create_texture_exportable(&inner_desc).unwrap();
            let texture = unsafe {
                self.inner
                    .create_texture_from_hal::<Dx12Api>(dxtexture, &texture)
            };

            Some(ExportableTexture {
                texture,
                size,
                row_stride: 512,
                alignment,
            })
        } else {
            // TODO: This should never happen and we really should panic here
            self.inner
                .create_texture_exportable(adapter, queue, texture)
        }
    }

    pub fn create_dx11_texture(
        &self,
        texture: &wgpu::TextureDescriptor<'_>,
    ) -> Option<ID3D11Texture2D> {
        let inner_desc = D3D11_TEXTURE2D_DESC {
            Width: texture.size.width,
            Height: texture.size.height,
            MipLevels: texture.mip_level_count,
            ArraySize: texture.array_layer_count(),
            Format: map_texture_format(texture.format),
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: texture.sample_count,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,

            //TODO: What's the bind flags for copying a texture?
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32 | D3D11_BIND_RENDER_TARGET.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: D3D11_RESOURCE_MISC_SHARED.0 as u32,
        };

        let mut texture = None;
        unsafe {
            self.d3d11_dev
                .CreateTexture2D(&inner_desc, None, Some(&mut texture))
                .unwrap();
        }

        texture
    }
}

/// A texture that has been exported as a D3D12 resource handle.
#[derive(Debug, Clone)]
pub struct ExportedTexture {
    texture: wgpu::Texture,
}

impl ExportedTexture {
    pub fn from_exportable(exportable: &ExportableTexture) -> Result<Self, OurError> {
        Ok(Self {
            texture: exportable.texture.clone(),
        })
    }

    pub fn as_id3d12_resource(&self) -> ID3D12Resource {
        unsafe {
            self.texture
                .as_hal::<wgpu_hal::dx12::Api>()
                .unwrap()
                .raw_resource()
                .clone()
        }
    }

    /// Consume the WGPU texture, producing a D3D11 resource.
    ///
    /// NOTE: Calling this function invalidates the associated WGPU texture.
    /// Probably. I don't know yet, but D3D11On12 makes me change the resource
    /// state behind WGPU's back and that's gotta piss off the GPU bureaucrats
    /// somehow
    pub fn into_d3d11_resource(
        self,
        device: &ExtendedDevice,
        _queue: &wgpu::Queue,
    ) -> Result<ID3D11Resource, OurError> {
        let resource = self.as_id3d12_resource();

        let flags11 = D3D11_RESOURCE_FLAGS::default();
        let mut resource11 = None;

        unsafe {
            device
                .d3d11on12_dev
                .CreateWrappedResource(
                    &resource,
                    &flags11,
                    D3D12_RESOURCE_STATE_COPY_SOURCE,
                    D3D12_RESOURCE_STATE_PRESENT,
                    &mut resource11,
                )
                .unwrap();
        }

        Ok(resource11.unwrap())
    }

    pub fn as_nt_share_handle(&self, device: &ExtendedDevice) -> Result<HANDLE, OurError> {
        let dx12_hal = unsafe { device.inner.as_hal::<wgpu_hal::api::Dx12>() }.unwrap();
        let dx12_device = dx12_hal.raw_device();
        let d3dresource = self.as_id3d12_resource();
        let share_handle =
            unsafe { dx12_device.CreateSharedHandle(&d3dresource, None, GENERIC_ALL.0, None)? };

        Ok(share_handle)
    }
}
