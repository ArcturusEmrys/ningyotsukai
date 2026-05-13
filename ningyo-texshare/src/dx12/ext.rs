use wgpu_hal::dx12::{Device, Texture};
use wgpu_hal::{DeviceError, TextureDescriptor};

use windows::Win32::Graphics::{Direct3D12, Dxgi};

use crate::dx12::conv;
use crate::error::Error as OurError;

pub trait DeviceExt {
    /// Create an exportable texture with all of the extensions necessary to be
    /// exported from this device's context.
    fn create_texture_exportable(
        &self,
        texture: &TextureDescriptor<'_>,
    ) -> Result<(Texture, u64, u64), OurError>;
}

impl DeviceExt for Device {
    fn create_texture_exportable(
        &self,
        desc: &TextureDescriptor<'_>,
    ) -> Result<(Texture, u64, u64), OurError> {
        //TODO: Google says we need to use the following flags:
        // D3D12_RESOURCE_FLAG_ALLOW_SIMULTANEOUS_ACCESS
        // D3D12_HEAP_FLAG_SHARED

        let mut raw_desc = Direct3D12::D3D12_RESOURCE_DESC {
            Dimension: conv::map_texture_dimension(desc.dimension),
            Alignment: 0,
            Width: desc.size.width as u64,
            Height: desc.size.height,
            DepthOrArraySize: desc.size.depth_or_array_layers as u16,
            MipLevels: desc.mip_level_count as u16,
            Format: conv::map_texture_format_for_resource(
                desc.format,
                desc.usage,
                !desc.view_formats.is_empty(),
                false,
                /* TODO: There's a shared private capability that can set this
                self.shared
                    .private_caps
                    .casting_fully_typed_format_supported, */
            ),
            SampleDesc: Dxgi::Common::DXGI_SAMPLE_DESC {
                Count: desc.sample_count,
                Quality: 0,
            },
            Layout: Direct3D12::D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: conv::map_texture_usage_to_resource_flags(desc.usage),
        };
        let info = unsafe { self.raw_device().GetResourceAllocationInfo(0, &[raw_desc]) };

        let mut architecture = Direct3D12::D3D12_FEATURE_DATA_ARCHITECTURE::default();
        unsafe {
            self.raw_device().CheckFeatureSupport(
                Direct3D12::D3D12_FEATURE_ARCHITECTURE,
                &mut architecture as *mut _ as *mut _,
                size_of::<Direct3D12::D3D12_FEATURE_DATA_ARCHITECTURE>() as u32,
            )?
        }

        let heap_properties = Direct3D12::D3D12_HEAP_PROPERTIES {
            Type: Direct3D12::D3D12_HEAP_TYPE_CUSTOM,
            CPUPageProperty: Direct3D12::D3D12_CPU_PAGE_PROPERTY_NOT_AVAILABLE,
            // TODO: This is supposed to be L1 for UMA devices, L0 for discrete.
            // We aren't allowed to know that so... guess? If this doesn't work,
            // try L0, it at least won't crash UMA
            MemoryPoolPreference: match architecture.UMA.as_bool() {
                // Translator's note: UMA means HORSE
                false => Direct3D12::D3D12_MEMORY_POOL_L1,
                true => Direct3D12::D3D12_MEMORY_POOL_L0,
            },
            CreationNodeMask: 0,
            VisibleNodeMask: 0,
        };

        let mut resource = None;

        // NOTE: We currently fail with "The parameter is incorrect" at this
        // point, even though we're handing DX12 the same properties the Wgpu
        // code does?!

        unsafe {
            self.raw_device().CreateCommittedResource(
                &heap_properties,
                Direct3D12::D3D12_HEAP_FLAG_SHARED
                    | Direct3D12::D3D12_HEAP_FLAG_ALLOW_ALL_BUFFERS_AND_TEXTURES,
                &raw_desc,
                Direct3D12::D3D12_RESOURCE_STATE_COMMON,
                None,
                &mut resource,
            )?
        }

        let resource = resource.ok_or(DeviceError::Unexpected)?;

        // TODO: We can't get at the performance counters.
        //self.counters.textures.add(1);

        Ok(unsafe {
            (
                Device::texture_from_raw(
                    resource,
                    desc.format,
                    desc.dimension,
                    desc.size,
                    desc.mip_level_count,
                    desc.sample_count,
                ),
                info.SizeInBytes,
                info.Alignment,
            )
        })
    }
}
