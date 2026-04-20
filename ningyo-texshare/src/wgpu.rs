//! Glue functions to generate WGPU objects holding our required API
//! extensions.

use crate::texture::ExportableTexture;
use crate::vulkan;
use crate::vulkan::AdapterExt as VulkanAdapterExt;
use crate::vulkan::DeviceExt as VulkanDeviceExt;
use wgpu::{
    Adapter, Device, DeviceDescriptor, Instance, InstanceDescriptor, Queue, RequestDeviceError,
    TextureDescriptor,
};
use wgpu_core::device::DeviceError as CoreDeviceError;
use wgpu_core::instance::RequestDeviceError as CoreRequestDeviceError;
use wgpu_hal::InstanceError;
use wgpu_hal::vulkan::Api as VulkanApi;

pub trait InstanceExt {
    fn new_with_extensions(desc: InstanceDescriptor) -> Result<Instance, InstanceError>;
}

impl InstanceExt for Instance {
    fn new_with_extensions(desc: InstanceDescriptor) -> Result<Instance, InstanceError> {
        // TODO: Use desc's backend flags to choose a backend once we have all of
        // them set up.
        if desc.backends.contains(wgpu::Backends::VULKAN) {
            unsafe {
                Ok(Instance::from_hal::<VulkanApi>(vulkan::instance_init(
                    &vulkan::instance_descriptor_convert("ningyo-gtk-wgpu", desc),
                )?))
            }
        } else {
            Ok(Instance::new(desc))
        }
    }
}

pub trait AdapterExt {
    #[allow(async_fn_in_trait)]
    async fn request_device_with_extensions(
        &self,
        desc: &DeviceDescriptor<'_>,
    ) -> Result<(Device, Queue), RequestDeviceError>;
}

impl AdapterExt for Adapter {
    async fn request_device_with_extensions(
        &self,
        desc: &DeviceDescriptor<'_>,
    ) -> Result<(Device, Queue), RequestDeviceError> {
        // SAFETY: We aren't going to destroy the adapter.
        if let Some(vulkan_adapter) = unsafe { self.as_hal::<VulkanApi>() } {
            unsafe {
                let open_device = vulkan_adapter
                    .open_with_extensions(
                        desc.required_features,
                        &desc.required_limits,
                        &desc.memory_hints,
                    )
                    .map_err(|e| {
                        RequestDeviceError::from(CoreRequestDeviceError::Device(
                            CoreDeviceError::from_hal(e),
                        ))
                    })?;

                Ok(self.create_device_from_hal(open_device, desc)?)
            }
        } else {
            //fall back to standard
            self.request_device(desc).await
        }
    }
}

pub trait DeviceExt {
    /// Create an exportable texture that can be shared with external users.
    ///
    /// Adapter must be the same adapter that was used to get this device.
    ///
    /// Texture is guaranteed to be exportable to the following formats per
    /// platform:
    ///
    /// * Linux: DMA-BUF (via Vulkan)
    ///
    /// Returns None if no texture export backend is available or if the
    /// requested texture descriptor is incompatible with texture sharing.
    fn create_texture_exportable(
        &self,
        adapter: &wgpu::Adapter,
        queue: &wgpu::Queue,
        texture: &TextureDescriptor<'_>,
    ) -> Option<ExportableTexture>;
}

fn map_texture_usage(
    usage: wgpu::TextureUsages,
    aspect: wgpu_hal::FormatAspects,
    flags: wgpu::TextureFormatFeatureFlags,
) -> wgpu::TextureUses {
    let mut u = wgpu::TextureUses::empty();
    u.set(
        wgpu::TextureUses::COPY_SRC,
        usage.contains(wgpu::TextureUsages::COPY_SRC),
    );
    u.set(
        wgpu::TextureUses::COPY_DST,
        usage.contains(wgpu::TextureUsages::COPY_DST),
    );
    u.set(
        wgpu::TextureUses::RESOURCE,
        usage.contains(wgpu::TextureUsages::TEXTURE_BINDING),
    );
    if usage.contains(wgpu::TextureUsages::STORAGE_BINDING) {
        u.set(
            wgpu::TextureUses::STORAGE_READ_ONLY,
            flags.contains(wgpu::TextureFormatFeatureFlags::STORAGE_READ_ONLY),
        );
        u.set(
            wgpu::TextureUses::STORAGE_WRITE_ONLY,
            flags.contains(wgpu::TextureFormatFeatureFlags::STORAGE_WRITE_ONLY),
        );
        u.set(
            wgpu::TextureUses::STORAGE_READ_WRITE,
            flags.contains(wgpu::TextureFormatFeatureFlags::STORAGE_READ_WRITE),
        );
    }
    let is_color = aspect.intersects(
        wgpu_hal::FormatAspects::COLOR
            | wgpu_hal::FormatAspects::PLANE_0
            | wgpu_hal::FormatAspects::PLANE_1
            | wgpu_hal::FormatAspects::PLANE_2,
    );
    u.set(
        wgpu::TextureUses::COLOR_TARGET,
        usage.contains(wgpu::TextureUsages::RENDER_ATTACHMENT) && is_color,
    );
    u.set(
        wgpu::TextureUses::DEPTH_STENCIL_READ | wgpu::TextureUses::DEPTH_STENCIL_WRITE,
        usage.contains(wgpu::TextureUsages::RENDER_ATTACHMENT) && !is_color,
    );
    u.set(
        wgpu::TextureUses::STORAGE_ATOMIC,
        usage.contains(wgpu::TextureUsages::STORAGE_ATOMIC),
    );
    u.set(
        wgpu::TextureUses::TRANSIENT,
        usage.contains(wgpu::TextureUsages::TRANSIENT),
    );
    u
}

fn map_texture_usage_for_texture(
    desc: &TextureDescriptor,
    format_features: &wgpu::TextureFormatFeatures,
) -> wgpu::TextureUses {
    // Enforce having COPY_DST/DEPTH_STENCIL_WRITE/COLOR_TARGET otherwise we
    // wouldn't be able to initialize the texture.
    map_texture_usage(desc.usage, desc.format.into(), format_features.flags)
        | if desc.format.is_depth_stencil_format() {
            wgpu::TextureUses::DEPTH_STENCIL_WRITE
        } else if desc.usage.contains(wgpu::TextureUsages::COPY_DST) {
            wgpu::TextureUses::COPY_DST // (set already)
        } else {
            // Use COPY_DST only if we can't use COLOR_TARGET
            if format_features
                .allowed_usages
                .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
                && desc.dimension == wgpu::TextureDimension::D2
            // Render targets dimension must be 2d
            {
                wgpu::TextureUses::COLOR_TARGET
            } else {
                wgpu::TextureUses::COPY_DST
            }
        }
}

impl DeviceExt for Device {
    fn create_texture_exportable(
        &self,
        adapter: &wgpu::Adapter,
        queue: &wgpu::Queue,
        texture: &TextureDescriptor<'_>,
    ) -> Option<ExportableTexture> {
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

        if let Some(vkdevice) = unsafe { self.as_hal::<VulkanApi>() } {
            let (hal_texture, mem_req, subresource_layout) =
                vkdevice.create_texture_exportable(&inner_desc).unwrap();
            //TODO: Really? We're going to unwrap!?
            let texture =
                unsafe { self.create_texture_from_hal::<VulkanApi>(hal_texture, &texture) };

            //TODO: Once we find a DMA-BUF format that WORKS, check if this is
            //still necessary or if normal clears will work.
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    aspect: wgpu::TextureAspect::All,
                    mip_level: 0,
                    texture: &texture,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                },
                &[0, 0, 0, 0],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: None,
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );

            Some(ExportableTexture {
                texture,
                size: mem_req.size,
                row_stride: subresource_layout.row_pitch,
                alignment: mem_req.alignment,
            })
        } else {
            //TODO: Actually implement texture export for DX12 and Metal.
            let texture = self.create_texture(texture);

            //TODO: These values are bad guesses.
            Some(ExportableTexture {
                texture: texture.clone(),
                size: texture.width() as u64 * texture.height() as u64 * 4 as u64,
                row_stride: texture.width() as u64 * 4 as u64,
                alignment: 1,
            })
        }
    }
}
