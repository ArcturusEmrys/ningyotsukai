use crate::error::Error as OurError;
use crate::vulkan::conv;
use ash::vk;
use wgpu_hal::TextureDescriptor;
use wgpu_hal::vulkan::Device;

/// Given a Vulkan format and intended usage, find all compatible DRM modifiers.
///
/// Returns a list of modifier IDs and plane counts.
#[cfg(target_os = "linux")]
fn find_compatible_drm_modifier(
    device: &Device,
    format: vk::Format,
    usage: vk::ImageUsageFlags,
) -> Vec<(u64, u32)> {
    let mut out_format_properties = vk::FormatProperties2::default();
    let mut out_drm_format = vk::DrmFormatModifierPropertiesListEXT::default();

    out_format_properties = out_format_properties.push_next(&mut out_drm_format);

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

    #[cfg(feature = "chatty_debug")]
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

pub fn create_image_without_memory(
    device: &Device,
    desc: &TextureDescriptor,
    tiling: vk::ImageTiling,
    external_memory_image_create_info: Option<&mut vk::ExternalMemoryImageCreateInfo>,
) -> Result<(vk::Image, vk::MemoryRequirements), OurError> {
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

    let original_format = conv::map_texture_format(desc.format);
    let vk_view_formats = vec![];
    if !desc.view_formats.is_empty() {
        raw_flags |= vk::ImageCreateFlags::MUTABLE_FORMAT;

        // TODO: We don't have access to wgpu_hal's private image format list,
        // either.
    }
    if desc.format.is_multi_planar_format() {
        raw_flags |= vk::ImageCreateFlags::MUTABLE_FORMAT | vk::ImageCreateFlags::EXTENDED_USAGE;
    }

    // TODO: These are all guesses.
    #[cfg(target_os = "windows")]
    {
        //raw_flags |= vk::ImageCreateFlags::MUTABLE_FORMAT | vk::ImageCreateFlags::BLOCK_TEXEL_VIEW_COMPATIBLE | vk::ImageCreateFlags::TYPE_2D_ARRAY_COMPATIBLE;
    }

    let mut vk_info = vk::ImageCreateInfo::default()
        .flags(raw_flags)
        .image_type(conv::map_texture_dimension(desc.dimension))
        .format(original_format)
        .extent(conv::map_copy_extent(&copy_size))
        .mip_levels(desc.mip_level_count)
        .array_layers(desc.array_layer_count())
        .samples(vk::SampleCountFlags::from_raw(desc.sample_count))
        .tiling(tiling)
        .usage(conv::map_texture_usage(desc.usage))
        .sharing_mode(vk::SharingMode::EXCLUSIVE)
        .initial_layout(vk::ImageLayout::UNDEFINED);

    #[cfg(feature = "chatty_debug")]
    dbg!(&vk_info);

    // DMA-BUF support: Always use linear layout images
    // TODO: NO, it turns out Intel does not like any of the image modifiers it says.
    // Google's AI is recommending I try ImageDrmFormatModifierExplicitCreateInfoEXT instead
    #[allow(unused)]
    let mut drm_extension = vk::ImageDrmFormatModifierListCreateInfoEXT::default();
    #[allow(unused)]
    let mut drm_extension_buffer: Vec<u64> = vec![];
    #[cfg(target_os = "linux")]
    {
        //NOTE: We specifically filter out LINEAR (mod 0) format as many GPUs
        //do not want to render to a linear texture (or at least, Intel doesn't
        //wanna)
        let compatible_modifier =
            find_compatible_drm_modifier(device, original_format, vk_info.usage)
                .iter()
                .filter(|(mods, planes)| *mods != 0 && *planes == 1)
                .map(|(mods, _planes)| *mods)
                .collect();

        #[cfg(feature = "chatty_debug")]
        dbg!(&compatible_modifier);

        drm_extension_buffer = compatible_modifier;
        drm_extension = drm_extension.drm_format_modifiers(&drm_extension_buffer);

        #[cfg(feature = "chatty_debug")]
        dbg!(&drm_extension);

        vk_info = vk_info.push_next(&mut drm_extension);
    }

    let mut format_list_info = vk::ImageFormatListCreateInfo::default();
    if !vk_view_formats.is_empty() {
        format_list_info = format_list_info.view_formats(&vk_view_formats);
        #[cfg(feature = "chatty_debug")]
        dbg!(&format_list_info);
        vk_info = vk_info.push_next(&mut format_list_info);
    }

    if let Some(ext_info) = external_memory_image_create_info {
        #[cfg(feature = "chatty_debug")]
        dbg!(&ext_info);
        vk_info = vk_info.push_next(ext_info);
    }

    #[cfg(feature = "chatty_debug")]
    dbg!(&vk_info);

    let raw = unsafe { device.raw_device().create_image(&vk_info, None) }
        .map_err(|e| OurError::from(e))?;
    let mut req = unsafe { device.raw_device().get_image_memory_requirements(raw) };

    if desc.usage.contains(wgpu::TextureUses::TRANSIENT) {
        let mem_type_index = conv::find_memory_type_index(
            device,
            req.memory_type_bits,
            vk::MemoryPropertyFlags::LAZILY_ALLOCATED,
        );
        if let Some(mem_type_index) = mem_type_index {
            req.memory_type_bits = 1 << mem_type_index;
        }
    }

    Ok((raw, req))
}
