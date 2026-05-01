use wgpu_hal::vulkan::TextureMemory;

use ash::ext::image_drm_format_modifier;
use ash::khr::external_memory_fd;
use ash::vk::{
    ExternalMemoryHandleTypeFlags, ImageAspectFlags, ImageDrmFormatModifierPropertiesEXT,
    ImageSubresource, MemoryGetFdInfoKHR, StructureType,
};
use std::marker::PhantomData;
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd};
use std::ptr::null;

use crate::error::Error as OurError;
use crate::texture::ExportableTexture;

/// An extended device that has the necessary extensions for texture sharing.
#[derive(Clone)]
pub struct ExtendedDevice {
    inner: wgpu::Device,
}

impl ExtendedDevice {
    pub fn wrap(device: wgpu::Device) -> Self {
        Self { inner: device }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.inner
    }

    pub fn create_texture_exportable(
        &self,
        adapter: &wgpu::Adapter,
        queue: &wgpu::Queue,
        texture: &wgpu::TextureDescriptor<'_>,
    ) -> Option<ExportableTexture> {
        self.inner
            .create_texture_exportable(adapter, queue, texture)
    }
}

//TODO: Actually write a test case for all of this code.

pub struct ExportedTexture {
    texture: wgpu::Texture,
    offset: u64,
    stride: u64,
    fd: OwnedFd,
    modifier: u64,
}

impl ExportedTexture {
    pub fn export_to_dmabuf(
        device: &wgpu::Device,
        texture: &ExportableTexture,
    ) -> Result<ExportedTexture, OurError> {
        unsafe {
            let Some(inner_texture) = texture.texture().as_hal::<wgpu_hal::api::Vulkan>() else {
                return Err(OurError::WrongAPIError);
            };
            let device = device.as_hal::<wgpu_hal::api::Vulkan>().unwrap();
            let instance = device.shared_instance();
            if instance
                .extensions()
                .iter()
                .find(|ext| ext.to_bytes() == "VK_KHR_external_memory_capabilities".as_bytes())
                .is_none()
            {
                return Err(OurError::MissingExtension);
            }

            let memory = match inner_texture.memory() {
                TextureMemory::Allocation(alloc) => alloc.memory(),
                TextureMemory::Dedicated(memory) => *memory,
                TextureMemory::External => return Err(OurError::OpaqueExport),
            };

            let ext_mem_api =
                external_memory_fd::Device::new(instance.raw_instance(), device.raw_device());

            let layout = device.raw_device().get_image_subresource_layout(
                inner_texture.raw_handle(),
                ImageSubresource {
                    aspect_mask: ImageAspectFlags::MEMORY_PLANE_0_EXT,
                    mip_level: 0,
                    array_layer: 0,
                },
            );

            let fd = ext_mem_api.get_memory_fd(&MemoryGetFdInfoKHR {
                s_type: StructureType::MEMORY_GET_FD_INFO_KHR,
                p_next: null(),
                memory,
                handle_type: ExternalMemoryHandleTypeFlags::DMA_BUF_EXT,
                _marker: PhantomData,
            })?;

            if fd == -1 {
                return Err(OurError::InvalidFd);
            }

            let image_drm_format_api = image_drm_format_modifier::Device::new(
                instance.raw_instance(),
                device.raw_device(),
            );
            let mut out_image_drm = ImageDrmFormatModifierPropertiesEXT::default();
            image_drm_format_api.get_image_drm_format_modifier_properties(
                inner_texture.raw_handle(),
                &mut out_image_drm,
            )?;

            Ok(ExportedTexture {
                texture: texture.texture().clone(),
                fd: OwnedFd::from_raw_fd(fd),
                offset: layout.offset,
                stride: layout.row_pitch,
                modifier: out_image_drm.drm_format_modifier,
            })
        }
    }

    /// Get the fd for this exported texture.
    pub fn fd<'a>(&'a self) -> BorrowedFd<'a> {
        self.fd.as_fd()
    }

    /// Get the texture that was exported.
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// Get the LinuxDRM / DMA-BUF format modifier for this texture.
    pub fn modifier(&self) -> u64 {
        self.modifier
    }

    pub fn into_gdk_texture(self) -> Result<gdk4::Texture, Box<dyn std::error::Error>> {
        //TODO: Add an API for re-exporting a texture to GDK.
        //There's a builder function set_update_texture
        unsafe {
            #[cfg(feature = "chatty_debug")]
            {
                dbg!(self.texture.width());
                dbg!(self.texture.height());
                dbg!(self.fd.as_raw_fd());
                dbg!(self.offset);
                dbg!(self.stride);
                dbg!(self.texture.format());
            }
            Ok(gdk4::DmabufTextureBuilder::new()
                .set_width(self.texture.width())
                .set_height(self.texture.height())
                .set_fd(0, self.fd.as_raw_fd())
                .set_offset(0, self.offset as u32)
                .set_stride(0, self.stride as u32)
                .set_fourcc(match self.texture.format() {
                    //TODO: This is endian-dependent (thanks, Lie-nus).
                    wgpu::TextureFormat::Rgba8Uint
                    | wgpu::TextureFormat::Rgba8Unorm
                    | wgpu::TextureFormat::Rgba8Sint
                    | wgpu::TextureFormat::Rgba8Snorm
                    | wgpu::TextureFormat::Rgba8UnormSrgb => drm_fourcc::DrmFourcc::Abgr8888 as u32,
                    _ => {
                        return Err(OurError::InvalidFormat)?;
                    }
                })
                .set_modifier(self.modifier)
                .build_with_release_func(move || {
                    drop(self);
                })?)
        }
    }

    pub fn into_fd(self) -> (wgpu::Texture, OwnedFd) {
        (self.texture, self.fd)
    }
}
