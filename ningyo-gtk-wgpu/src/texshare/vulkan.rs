//! Custom Vulkan initialization that applies extensions necessary for texture
//! sharing.

use crate::error::Error as OurError;
use ash::vk;
use wgpu_hal::vulkan::{Adapter, Api, Device, Instance, Texture, TextureMemory};
use wgpu_hal::{
    CopyExtent, DeviceError, DynDevice, InstanceDescriptor, InstanceError, OpenDevice,
    TextureDescriptor,
};

/// Convert the high-level InstanceDescriptor into one wgpu_hal cares about.
pub fn instance_descriptor_convert(
    name: &str,
    desc: wgpu::InstanceDescriptor,
) -> InstanceDescriptor<'_> {
    InstanceDescriptor {
        name,
        backend_options: desc.backend_options,
        flags: desc.flags,
        memory_budget_thresholds: desc.memory_budget_thresholds,
        telemetry: None,
    }
}

/// Create a Vulkan instance with texture sharing extensions enabled.
pub fn instance_init(desc: &InstanceDescriptor<'_>) -> Result<Instance, InstanceError> {
    // SAFETY: i dunno lol wgpu_hal doesn't say anything about init
    unsafe {
        Instance::init_with_callback(
            desc,
            Some(Box::new(|args| {
                args.extensions.push(c"VK_KHR_external_memory_capabilities");
            })),
        )
    }
}

/// Trait for adapters that adds our extensions to them.
pub trait AdapterExt {
    unsafe fn open_with_extensions<'a>(
        &self,
        features: wgpu::Features,
        memory_hints: &wgpu::MemoryHints,
    ) -> Result<OpenDevice<Api>, DeviceError>;
}

impl AdapterExt for Adapter {
    unsafe fn open_with_extensions<'a>(
        &self,
        features: wgpu::Features,
        memory_hints: &wgpu::MemoryHints,
    ) -> Result<OpenDevice<Api>, DeviceError> {
        unsafe {
            self.open_with_callback(
                features,
                memory_hints,
                Some(Box::new(|args| {
                    args.extensions.push(c"VK_KHR_external_memory");

                    #[cfg(target_os = "linux")]
                    {
                        args.extensions.push(c"VK_KHR_external_memory_fd");
                        args.extensions.push(c"VK_EXT_image_drm_format_modifier");
                    }
                })),
            )
        }
    }
}

// TODO: check all these map_functions for every major wgpu version

pub fn map_texture_format(format: wgpu::TextureFormat) -> vk::Format {
    use ash::vk::Format as F;
    use wgpu::TextureFormat as Tf;
    use wgpu::{AstcBlock, AstcChannel};
    //TODO: This is copied from wgpu_hal::vulkan, but we don't have access to
    //the library's private_caps structure. So instead we pick the most
    //pessimistic possible values.
    match format {
        Tf::R8Unorm => F::R8_UNORM,
        Tf::R8Snorm => F::R8_SNORM,
        Tf::R8Uint => F::R8_UINT,
        Tf::R8Sint => F::R8_SINT,
        Tf::R16Uint => F::R16_UINT,
        Tf::R16Sint => F::R16_SINT,
        Tf::R16Unorm => F::R16_UNORM,
        Tf::R16Snorm => F::R16_SNORM,
        Tf::R16Float => F::R16_SFLOAT,
        Tf::Rg8Unorm => F::R8G8_UNORM,
        Tf::Rg8Snorm => F::R8G8_SNORM,
        Tf::Rg8Uint => F::R8G8_UINT,
        Tf::Rg8Sint => F::R8G8_SINT,
        Tf::Rg16Unorm => F::R16G16_UNORM,
        Tf::Rg16Snorm => F::R16G16_SNORM,
        Tf::R32Uint => F::R32_UINT,
        Tf::R32Sint => F::R32_SINT,
        Tf::R32Float => F::R32_SFLOAT,
        Tf::Rg16Uint => F::R16G16_UINT,
        Tf::Rg16Sint => F::R16G16_SINT,
        Tf::Rg16Float => F::R16G16_SFLOAT,
        Tf::Rgba8Unorm => F::R8G8B8A8_UNORM,
        Tf::Rgba8UnormSrgb => F::R8G8B8A8_SRGB,
        Tf::Bgra8UnormSrgb => F::B8G8R8A8_SRGB,
        Tf::Rgba8Snorm => F::R8G8B8A8_SNORM,
        Tf::Bgra8Unorm => F::B8G8R8A8_UNORM,
        Tf::Rgba8Uint => F::R8G8B8A8_UINT,
        Tf::Rgba8Sint => F::R8G8B8A8_SINT,
        Tf::Rgb10a2Uint => F::A2B10G10R10_UINT_PACK32,
        Tf::Rgb10a2Unorm => F::A2B10G10R10_UNORM_PACK32,
        Tf::Rg11b10Ufloat => F::B10G11R11_UFLOAT_PACK32,
        Tf::R64Uint => F::R64_UINT,
        Tf::Rg32Uint => F::R32G32_UINT,
        Tf::Rg32Sint => F::R32G32_SINT,
        Tf::Rg32Float => F::R32G32_SFLOAT,
        Tf::Rgba16Uint => F::R16G16B16A16_UINT,
        Tf::Rgba16Sint => F::R16G16B16A16_SINT,
        Tf::Rgba16Unorm => F::R16G16B16A16_UNORM,
        Tf::Rgba16Snorm => F::R16G16B16A16_SNORM,
        Tf::Rgba16Float => F::R16G16B16A16_SFLOAT,
        Tf::Rgba32Uint => F::R32G32B32A32_UINT,
        Tf::Rgba32Sint => F::R32G32B32A32_SINT,
        Tf::Rgba32Float => F::R32G32B32A32_SFLOAT,
        Tf::Depth32Float => F::D32_SFLOAT,
        Tf::Depth32FloatStencil8 => F::D32_SFLOAT_S8_UINT,
        Tf::Depth24Plus => F::D32_SFLOAT,
        Tf::Depth24PlusStencil8 => F::D32_SFLOAT_S8_UINT,
        Tf::Stencil8 => F::D32_SFLOAT_S8_UINT,
        Tf::Depth16Unorm => F::D16_UNORM,
        Tf::NV12 => F::G8_B8R8_2PLANE_420_UNORM,
        Tf::P010 => F::G10X6_B10X6R10X6_2PLANE_420_UNORM_3PACK16,
        Tf::Rgb9e5Ufloat => F::E5B9G9R9_UFLOAT_PACK32,
        Tf::Bc1RgbaUnorm => F::BC1_RGBA_UNORM_BLOCK,
        Tf::Bc1RgbaUnormSrgb => F::BC1_RGBA_SRGB_BLOCK,
        Tf::Bc2RgbaUnorm => F::BC2_UNORM_BLOCK,
        Tf::Bc2RgbaUnormSrgb => F::BC2_SRGB_BLOCK,
        Tf::Bc3RgbaUnorm => F::BC3_UNORM_BLOCK,
        Tf::Bc3RgbaUnormSrgb => F::BC3_SRGB_BLOCK,
        Tf::Bc4RUnorm => F::BC4_UNORM_BLOCK,
        Tf::Bc4RSnorm => F::BC4_SNORM_BLOCK,
        Tf::Bc5RgUnorm => F::BC5_UNORM_BLOCK,
        Tf::Bc5RgSnorm => F::BC5_SNORM_BLOCK,
        Tf::Bc6hRgbUfloat => F::BC6H_UFLOAT_BLOCK,
        Tf::Bc6hRgbFloat => F::BC6H_SFLOAT_BLOCK,
        Tf::Bc7RgbaUnorm => F::BC7_UNORM_BLOCK,
        Tf::Bc7RgbaUnormSrgb => F::BC7_SRGB_BLOCK,
        Tf::Etc2Rgb8Unorm => F::ETC2_R8G8B8_UNORM_BLOCK,
        Tf::Etc2Rgb8UnormSrgb => F::ETC2_R8G8B8_SRGB_BLOCK,
        Tf::Etc2Rgb8A1Unorm => F::ETC2_R8G8B8A1_UNORM_BLOCK,
        Tf::Etc2Rgb8A1UnormSrgb => F::ETC2_R8G8B8A1_SRGB_BLOCK,
        Tf::Etc2Rgba8Unorm => F::ETC2_R8G8B8A8_UNORM_BLOCK,
        Tf::Etc2Rgba8UnormSrgb => F::ETC2_R8G8B8A8_SRGB_BLOCK,
        Tf::EacR11Unorm => F::EAC_R11_UNORM_BLOCK,
        Tf::EacR11Snorm => F::EAC_R11_SNORM_BLOCK,
        Tf::EacRg11Unorm => F::EAC_R11G11_UNORM_BLOCK,
        Tf::EacRg11Snorm => F::EAC_R11G11_SNORM_BLOCK,
        Tf::Astc { block, channel } => match channel {
            AstcChannel::Unorm => match block {
                AstcBlock::B4x4 => F::ASTC_4X4_UNORM_BLOCK,
                AstcBlock::B5x4 => F::ASTC_5X4_UNORM_BLOCK,
                AstcBlock::B5x5 => F::ASTC_5X5_UNORM_BLOCK,
                AstcBlock::B6x5 => F::ASTC_6X5_UNORM_BLOCK,
                AstcBlock::B6x6 => F::ASTC_6X6_UNORM_BLOCK,
                AstcBlock::B8x5 => F::ASTC_8X5_UNORM_BLOCK,
                AstcBlock::B8x6 => F::ASTC_8X6_UNORM_BLOCK,
                AstcBlock::B8x8 => F::ASTC_8X8_UNORM_BLOCK,
                AstcBlock::B10x5 => F::ASTC_10X5_UNORM_BLOCK,
                AstcBlock::B10x6 => F::ASTC_10X6_UNORM_BLOCK,
                AstcBlock::B10x8 => F::ASTC_10X8_UNORM_BLOCK,
                AstcBlock::B10x10 => F::ASTC_10X10_UNORM_BLOCK,
                AstcBlock::B12x10 => F::ASTC_12X10_UNORM_BLOCK,
                AstcBlock::B12x12 => F::ASTC_12X12_UNORM_BLOCK,
            },
            AstcChannel::UnormSrgb => match block {
                AstcBlock::B4x4 => F::ASTC_4X4_SRGB_BLOCK,
                AstcBlock::B5x4 => F::ASTC_5X4_SRGB_BLOCK,
                AstcBlock::B5x5 => F::ASTC_5X5_SRGB_BLOCK,
                AstcBlock::B6x5 => F::ASTC_6X5_SRGB_BLOCK,
                AstcBlock::B6x6 => F::ASTC_6X6_SRGB_BLOCK,
                AstcBlock::B8x5 => F::ASTC_8X5_SRGB_BLOCK,
                AstcBlock::B8x6 => F::ASTC_8X6_SRGB_BLOCK,
                AstcBlock::B8x8 => F::ASTC_8X8_SRGB_BLOCK,
                AstcBlock::B10x5 => F::ASTC_10X5_SRGB_BLOCK,
                AstcBlock::B10x6 => F::ASTC_10X6_SRGB_BLOCK,
                AstcBlock::B10x8 => F::ASTC_10X8_SRGB_BLOCK,
                AstcBlock::B10x10 => F::ASTC_10X10_SRGB_BLOCK,
                AstcBlock::B12x10 => F::ASTC_12X10_SRGB_BLOCK,
                AstcBlock::B12x12 => F::ASTC_12X12_SRGB_BLOCK,
            },
            AstcChannel::Hdr => match block {
                AstcBlock::B4x4 => F::ASTC_4X4_SFLOAT_BLOCK_EXT,
                AstcBlock::B5x4 => F::ASTC_5X4_SFLOAT_BLOCK_EXT,
                AstcBlock::B5x5 => F::ASTC_5X5_SFLOAT_BLOCK_EXT,
                AstcBlock::B6x5 => F::ASTC_6X5_SFLOAT_BLOCK_EXT,
                AstcBlock::B6x6 => F::ASTC_6X6_SFLOAT_BLOCK_EXT,
                AstcBlock::B8x5 => F::ASTC_8X5_SFLOAT_BLOCK_EXT,
                AstcBlock::B8x6 => F::ASTC_8X6_SFLOAT_BLOCK_EXT,
                AstcBlock::B8x8 => F::ASTC_8X8_SFLOAT_BLOCK_EXT,
                AstcBlock::B10x5 => F::ASTC_10X5_SFLOAT_BLOCK_EXT,
                AstcBlock::B10x6 => F::ASTC_10X6_SFLOAT_BLOCK_EXT,
                AstcBlock::B10x8 => F::ASTC_10X8_SFLOAT_BLOCK_EXT,
                AstcBlock::B10x10 => F::ASTC_10X10_SFLOAT_BLOCK_EXT,
                AstcBlock::B12x10 => F::ASTC_12X10_SFLOAT_BLOCK_EXT,
                AstcBlock::B12x12 => F::ASTC_12X12_SFLOAT_BLOCK_EXT,
            },
        },
    }
}

pub fn map_texture_dimension(dim: wgpu::TextureDimension) -> vk::ImageType {
    match dim {
        wgpu::TextureDimension::D1 => vk::ImageType::TYPE_1D,
        wgpu::TextureDimension::D2 => vk::ImageType::TYPE_2D,
        wgpu::TextureDimension::D3 => vk::ImageType::TYPE_3D,
    }
}

pub fn map_copy_extent(extent: &CopyExtent) -> vk::Extent3D {
    vk::Extent3D {
        width: extent.width,
        height: extent.height,
        depth: extent.depth,
    }
}

pub fn map_texture_usage(usage: wgpu::TextureUses) -> vk::ImageUsageFlags {
    let mut flags = vk::ImageUsageFlags::empty();
    if usage.contains(wgpu::TextureUses::COPY_SRC) {
        flags |= vk::ImageUsageFlags::TRANSFER_SRC;
    }
    if usage.contains(wgpu::TextureUses::COPY_DST) {
        flags |= vk::ImageUsageFlags::TRANSFER_DST;
    }
    if usage.contains(wgpu::TextureUses::RESOURCE) {
        flags |= vk::ImageUsageFlags::SAMPLED;
    }
    if usage.contains(wgpu::TextureUses::COLOR_TARGET) {
        flags |= vk::ImageUsageFlags::COLOR_ATTACHMENT;
    }
    if usage
        .intersects(wgpu::TextureUses::DEPTH_STENCIL_READ | wgpu::TextureUses::DEPTH_STENCIL_WRITE)
    {
        flags |= vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT;
    }
    if usage.intersects(
        wgpu::TextureUses::STORAGE_READ_ONLY
            | wgpu::TextureUses::STORAGE_WRITE_ONLY
            | wgpu::TextureUses::STORAGE_READ_WRITE
            | wgpu::TextureUses::STORAGE_ATOMIC,
    ) {
        flags |= vk::ImageUsageFlags::STORAGE;
    }
    if usage.contains(wgpu::TextureUses::TRANSIENT) {
        flags |= vk::ImageUsageFlags::TRANSIENT_ATTACHMENT;
    }
    flags
}

fn find_memory_type_index(
    device: &Device,
    type_bits_req: u32,
    flags_req: vk::MemoryPropertyFlags,
) -> Option<usize> {
    let mem_properties = unsafe {
        device
            .shared_instance()
            .raw_instance()
            .get_physical_device_memory_properties(device.raw_physical_device())
    };

    // https://registry.khronos.org/vulkan/specs/1.3-extensions/man/html/VkPhysicalDeviceMemoryProperties.html
    for (i, mem_ty) in mem_properties.memory_types_as_slice().iter().enumerate() {
        let types_bits = 1 << i;
        let is_required_memory_type = type_bits_req & types_bits != 0;
        let has_required_properties = mem_ty.property_flags & flags_req == flags_req;
        if is_required_memory_type && has_required_properties {
            return Some(i);
        }
    }

    None
}

fn map_err(err: vk::Result) -> DeviceError {
    // We don't use VK_EXT_image_compression_control
    // VK_ERROR_COMPRESSION_EXHAUSTED_EXT
    match err {
        vk::Result::ERROR_OUT_OF_HOST_MEMORY | vk::Result::ERROR_OUT_OF_DEVICE_MEMORY => {
            DeviceError::OutOfMemory
        }
        //TODO: wgpu_hal has an option to make these errors into panics.
        _e => DeviceError::Unexpected,
    }
}

/// Given a Vulkan format and intended usage, find all compatible DRM modifiers.
///
/// Returns a list of modifier IDs and plane counts.
fn find_compatible_drm_modifier(
    device: &Device,
    format: vk::Format,
    usage: vk::ImageUsageFlags,
) -> Vec<(u64, u32)> {
    let mut out_format_properties = vk::FormatProperties2::default();
    let mut out_drm_format = vk::DrmFormatModifierPropertiesListEXT::default();

    #[cfg(target_os = "linux")]
    {
        out_format_properties = out_format_properties.push_next(&mut out_drm_format);
    }

    unsafe {
        device
            .shared_instance()
            .raw_instance()
            .get_physical_device_format_properties2(
                device.raw_physical_device(),
                format,
                &mut out_format_properties,
            );
    }

    // NOTE: This is, officially, the dumbest way I've seen to get out
    // of having to hand the caller memory they need to dealloc.
    let mut out_drm_format_mods =
        Vec::with_capacity(out_drm_format.drm_format_modifier_count as usize);

    let mut out_format_properties = vk::FormatProperties2::default();
    let mut out_drm_format = vk::DrmFormatModifierPropertiesListEXT::default();

    out_drm_format.p_drm_format_modifier_properties = out_drm_format_mods.as_mut_ptr();
    out_drm_format.drm_format_modifier_count = out_drm_format_mods.capacity() as u32;

    out_format_properties = out_format_properties.push_next(&mut out_drm_format);

    // SAFETY: Vulkan better actually fill the damned array
    unsafe {
        device
            .shared_instance()
            .raw_instance()
            .get_physical_device_format_properties2(
                device.raw_physical_device(),
                format,
                &mut out_format_properties,
            );
        out_drm_format_mods.set_len(out_drm_format.drm_format_modifier_count as usize);
    }

    dbg!(&out_drm_format_mods);

    let mut desired_features = vk::FormatFeatureFlags::empty();
    if usage.contains(vk::ImageUsageFlags::TRANSFER_SRC) {
        desired_features |= vk::FormatFeatureFlags::TRANSFER_SRC;
    }
    if usage.contains(vk::ImageUsageFlags::TRANSFER_DST) {
        desired_features |= vk::FormatFeatureFlags::TRANSFER_DST;
    }
    if usage.contains(vk::ImageUsageFlags::SAMPLED) {
        desired_features |= vk::FormatFeatureFlags::SAMPLED_IMAGE;
    }
    if usage.contains(vk::ImageUsageFlags::STORAGE) {
        desired_features |= vk::FormatFeatureFlags::STORAGE_IMAGE;
    }
    if usage.contains(vk::ImageUsageFlags::COLOR_ATTACHMENT) {
        desired_features |= vk::FormatFeatureFlags::COLOR_ATTACHMENT;
    }
    if usage.contains(vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT) {
        desired_features |= vk::FormatFeatureFlags::DEPTH_STENCIL_ATTACHMENT;
    }
    //TODO: The above is not an exaustive list of usages.
    let mut ret = vec![];

    for modifier in out_drm_format_mods {
        if modifier
            .drm_format_modifier_tiling_features
            .contains(desired_features)
        {
            ret.push((
                modifier.drm_format_modifier,
                modifier.drm_format_modifier_plane_count,
            ));
        }
    }

    ret
}

fn create_image_without_memory(
    device: &Device,
    desc: &TextureDescriptor,
    tiling: vk::ImageTiling,
    external_memory_image_create_info: Option<&mut vk::ExternalMemoryImageCreateInfo>,
) -> Result<
    (
        vk::Image,
        vk::MemoryRequirements,
        vk::Format,
        vk::ImageType,
        vk::ImageUsageFlags,
        vk::ImageCreateFlags,
    ),
    DeviceError,
> {
    let copy_size = desc.copy_extent();

    let mut raw_flags = vk::ImageCreateFlags::empty();
    if desc.dimension == wgpu::TextureDimension::D3
        && desc.usage.contains(wgpu::TextureUses::COLOR_TARGET)
    {
        raw_flags |= vk::ImageCreateFlags::TYPE_2D_ARRAY_COMPATIBLE;
    }
    if desc.is_cube_compatible() {
        raw_flags |= vk::ImageCreateFlags::CUBE_COMPATIBLE;
    }

    let original_format = map_texture_format(desc.format);
    let vk_view_formats = vec![];
    if !desc.view_formats.is_empty() {
        raw_flags |= vk::ImageCreateFlags::MUTABLE_FORMAT;

        // TODO: We don't have access to wgpu_hal's private image format list,
        // either.
    }
    if desc.format.is_multi_planar_format() {
        raw_flags |= vk::ImageCreateFlags::MUTABLE_FORMAT | vk::ImageCreateFlags::EXTENDED_USAGE;
    }

    let mut vk_info = vk::ImageCreateInfo::default()
        .flags(raw_flags)
        .image_type(map_texture_dimension(desc.dimension))
        .format(original_format)
        .extent(map_copy_extent(&copy_size))
        .mip_levels(desc.mip_level_count)
        .array_layers(desc.array_layer_count())
        .samples(vk::SampleCountFlags::from_raw(desc.sample_count))
        .tiling(tiling)
        .usage(map_texture_usage(desc.usage))
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);

    // DMA-BUF support: Always use linear layout images
    // TODO: NO, it turns out Intel does not like any of the image modifiers it says.
    // Google's AI is recommending I try ImageDrmFormatModifierExplicitCreateInfoEXT instead
    #[allow(dead_code)]
    let mut drm_extension = vk::ImageDrmFormatModifierListCreateInfoEXT::default();
    let mut drm_extension_buffer = vec![];
    #[cfg(target_os = "linux")]
    {
        let compatible_modifier =
            find_compatible_drm_modifier(device, original_format, vk_info.usage)
                .iter()
                .filter(|(mods, planes)| *mods != 0 && *planes == 1)
                .map(|(mods, _planes)| *mods)
                .collect();

        dbg!(&compatible_modifier);

        drm_extension_buffer = compatible_modifier;
        drm_extension = drm_extension.drm_format_modifiers(&drm_extension_buffer);
        dbg!(&drm_extension);

        vk_info = vk_info.push_next(&mut drm_extension);
    }

    let mut format_list_info = vk::ImageFormatListCreateInfo::default();
    if !vk_view_formats.is_empty() {
        format_list_info = format_list_info.view_formats(&vk_view_formats);
        dbg!(&format_list_info);
        vk_info = vk_info.push_next(&mut format_list_info);
    }

    if let Some(ext_info) = external_memory_image_create_info {
        dbg!(&ext_info);
        vk_info = vk_info.push_next(ext_info);
    }

    dbg!(&vk_info);

    let raw = unsafe { device.raw_device().create_image(&vk_info, None) }.map_err(map_err)?;
    let mut req = unsafe { device.raw_device().get_image_memory_requirements(raw) };

    if desc.usage.contains(wgpu::TextureUses::TRANSIENT) {
        let mem_type_index = find_memory_type_index(
            device,
            req.memory_type_bits,
            vk::MemoryPropertyFlags::LAZILY_ALLOCATED,
        );
        if let Some(mem_type_index) = mem_type_index {
            req.memory_type_bits = 1 << mem_type_index;
        }
    }

    Ok((
        raw,
        req,
        vk_info.format,
        vk_info.image_type,
        vk_info.usage,
        vk_info.flags,
    ))
}

pub trait DeviceExt {
    fn create_texture_exportable(
        &self,
        texture: &TextureDescriptor<'_>,
    ) -> Result<Texture, OurError>;
}

impl DeviceExt for Device {
    fn create_texture_exportable(
        &self,
        texture: &TextureDescriptor<'_>,
    ) -> Result<Texture, OurError> {
        let mut handle_types = vk::ExternalMemoryHandleTypeFlags::default();
        let mut tiling = vk::ImageTiling::OPTIMAL;

        #[cfg(target_os = "linux")]
        {
            handle_types |= vk::ExternalMemoryHandleTypeFlags::DMA_BUF_EXT;
            tiling = vk::ImageTiling::DRM_FORMAT_MODIFIER_EXT;
        }

        let (image, mem_req, format, image_type, usage_flags, create_flags) =
            create_image_without_memory(
                self,
                texture,
                tiling,
                Some(&mut vk::ExternalMemoryImageCreateInfo::default().handle_types(handle_types)),
            )?;

        // Check that our chosen memory format can actually be exported.
        // TODO: Do I need to do this? Wouldn't it have already failed above?
        // TODO: Commented out this validation as it makes Intel drivers segfault
        // looking for a DRM format modifier struct that isn't there.
        // I could populate it but then I have to call 20 more things just to get
        // the winning modifier
        /*

        let mut format_info = vk::PhysicalDeviceImageFormatInfo2::default()
            .flags(create_flags)
            .format(format)
            .tiling(tiling)
            .ty(image_type)
            .usage(usage_flags);

        let mut drm_modifier_info = vk::PhysicalDeviceImageDrmFormatModifierInfoEXT::default()
            .drm_format_modifier();

        format_info = format_info.push_next(&mut drm_modifier_info);

        let mut out_fmt_props = vk::ImageFormatProperties2::default();

        let mut out_external_info = vk::ExternalImageFormatProperties::default();
        out_fmt_props = out_fmt_props.push_next(&mut out_external_info);

        unsafe {
            self.shared_instance()
                .raw_instance()
                .get_physical_device_image_format_properties2(
                    self.raw_physical_device(),
                    &format_info,
                    &mut out_fmt_props,
                )?;
        }

        eprintln!("{:?}", out_fmt_props);
        eprintln!("{:?}", out_external_info);

        //TODO: Seriously? This is the most expressive error I can return?!
        //TODO: Check for DEDICATED_ONLY? I mean, I want to do that anyway.
        if !out_external_info
            .external_memory_properties
            .compatible_handle_types
            .contains(handle_types)
            || !out_external_info
                .external_memory_properties
                .external_memory_features
                .contains(vk::ExternalMemoryFeatureFlags::EXPORTABLE)
        {
            //return Err(OurError::InvalidFormat);
        } */

        // Find a compatible memory heap to store into.
        let mem_props = unsafe {
            self.shared_instance()
                .raw_instance()
                .get_physical_device_memory_properties(self.raw_physical_device())
        };

        let mut desired_memory_type = None;
        for (memtype_index, memtype) in mem_props.memory_types_as_slice().iter().enumerate() {
            // Skip memory types not supported by the image's memory requirements.
            if mem_req.memory_type_bits >> memtype_index & 0x01 != 1 {
                continue;
            }

            // Skip non-device memory.
            // TODO: Do we care about the heap properties or do we just grab the first one?
            if memtype
                .property_flags
                .contains(vk::MemoryPropertyFlags::DEVICE_LOCAL)
            {
                desired_memory_type = Some(memtype_index);
            }
        }

        let Some(desired_memory_type) = desired_memory_type else {
            return Err(OurError::NoValidMemoryType);
        };

        let mut allocate_info = vk::MemoryAllocateInfo::default()
            .allocation_size(mem_req.size)
            .memory_type_index(desired_memory_type as u32);

        let mut external_memory_info =
            vk::ExportMemoryAllocateInfo::default().handle_types(handle_types);
        allocate_info = allocate_info.push_next(&mut external_memory_info);

        let mut dedicated_allocate_info = vk::MemoryDedicatedAllocateInfo::default().image(image);
        allocate_info = allocate_info.push_next(&mut dedicated_allocate_info);

        let memory = unsafe {
            self.raw_device()
                .allocate_memory(&allocate_info, None)
                .map_err(map_err)?
        };

        self.get_internal_counters()
            .texture_memory
            .add(mem_req.size as isize);

        unsafe {
            self.raw_device()
                .bind_image_memory(image, memory, 0)
                .map_err(map_err)?;

            dbg!(texture);
            Ok(self.texture_from_raw(image, texture, None, TextureMemory::Dedicated(memory)))
        }
    }
}
