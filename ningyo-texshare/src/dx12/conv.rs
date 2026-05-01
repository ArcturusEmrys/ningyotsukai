use wgpu_types as wgt;
use windows::Win32::Graphics::Direct3D12;
use windows::Win32::Graphics::Dxgi;

pub fn map_texture_dimension(dim: wgt::TextureDimension) -> Direct3D12::D3D12_RESOURCE_DIMENSION {
    match dim {
        wgt::TextureDimension::D1 => Direct3D12::D3D12_RESOURCE_DIMENSION_TEXTURE1D,
        wgt::TextureDimension::D2 => Direct3D12::D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        wgt::TextureDimension::D3 => Direct3D12::D3D12_RESOURCE_DIMENSION_TEXTURE3D,
    }
}

pub fn map_texture_format_failable(
    format: wgt::TextureFormat,
) -> Option<Dxgi::Common::DXGI_FORMAT> {
    use Dxgi::Common::*;
    use wgt::TextureFormat as Tf;

    Some(match format {
        Tf::R8Unorm => DXGI_FORMAT_R8_UNORM,
        Tf::R8Snorm => DXGI_FORMAT_R8_SNORM,
        Tf::R8Uint => DXGI_FORMAT_R8_UINT,
        Tf::R8Sint => DXGI_FORMAT_R8_SINT,
        Tf::R16Uint => DXGI_FORMAT_R16_UINT,
        Tf::R16Sint => DXGI_FORMAT_R16_SINT,
        Tf::R16Unorm => DXGI_FORMAT_R16_UNORM,
        Tf::R16Snorm => DXGI_FORMAT_R16_SNORM,
        Tf::R16Float => DXGI_FORMAT_R16_FLOAT,
        Tf::Rg8Unorm => DXGI_FORMAT_R8G8_UNORM,
        Tf::Rg8Snorm => DXGI_FORMAT_R8G8_SNORM,
        Tf::Rg8Uint => DXGI_FORMAT_R8G8_UINT,
        Tf::Rg8Sint => DXGI_FORMAT_R8G8_SINT,
        Tf::Rg16Unorm => DXGI_FORMAT_R16G16_UNORM,
        Tf::Rg16Snorm => DXGI_FORMAT_R16G16_SNORM,
        Tf::R32Uint => DXGI_FORMAT_R32_UINT,
        Tf::R32Sint => DXGI_FORMAT_R32_SINT,
        Tf::R32Float => DXGI_FORMAT_R32_FLOAT,
        Tf::Rg16Uint => DXGI_FORMAT_R16G16_UINT,
        Tf::Rg16Sint => DXGI_FORMAT_R16G16_SINT,
        Tf::Rg16Float => DXGI_FORMAT_R16G16_FLOAT,
        Tf::Rgba8Unorm => DXGI_FORMAT_R8G8B8A8_UNORM,
        Tf::Rgba8UnormSrgb => DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
        Tf::Bgra8UnormSrgb => DXGI_FORMAT_B8G8R8A8_UNORM_SRGB,
        Tf::Rgba8Snorm => DXGI_FORMAT_R8G8B8A8_SNORM,
        Tf::Bgra8Unorm => DXGI_FORMAT_B8G8R8A8_UNORM,
        Tf::Rgba8Uint => DXGI_FORMAT_R8G8B8A8_UINT,
        Tf::Rgba8Sint => DXGI_FORMAT_R8G8B8A8_SINT,
        Tf::Rgb9e5Ufloat => DXGI_FORMAT_R9G9B9E5_SHAREDEXP,
        Tf::Rgb10a2Uint => DXGI_FORMAT_R10G10B10A2_UINT,
        Tf::Rgb10a2Unorm => DXGI_FORMAT_R10G10B10A2_UNORM,
        Tf::Rg11b10Ufloat => DXGI_FORMAT_R11G11B10_FLOAT,
        Tf::R64Uint => DXGI_FORMAT_R32G32_UINT, // R64 emulated by R32G32
        Tf::Rg32Uint => DXGI_FORMAT_R32G32_UINT,
        Tf::Rg32Sint => DXGI_FORMAT_R32G32_SINT,
        Tf::Rg32Float => DXGI_FORMAT_R32G32_FLOAT,
        Tf::Rgba16Uint => DXGI_FORMAT_R16G16B16A16_UINT,
        Tf::Rgba16Sint => DXGI_FORMAT_R16G16B16A16_SINT,
        Tf::Rgba16Unorm => DXGI_FORMAT_R16G16B16A16_UNORM,
        Tf::Rgba16Snorm => DXGI_FORMAT_R16G16B16A16_SNORM,
        Tf::Rgba16Float => DXGI_FORMAT_R16G16B16A16_FLOAT,
        Tf::Rgba32Uint => DXGI_FORMAT_R32G32B32A32_UINT,
        Tf::Rgba32Sint => DXGI_FORMAT_R32G32B32A32_SINT,
        Tf::Rgba32Float => DXGI_FORMAT_R32G32B32A32_FLOAT,
        Tf::Stencil8 => DXGI_FORMAT_D24_UNORM_S8_UINT,
        Tf::Depth16Unorm => DXGI_FORMAT_D16_UNORM,
        Tf::Depth24Plus => DXGI_FORMAT_D24_UNORM_S8_UINT,
        Tf::Depth24PlusStencil8 => DXGI_FORMAT_D24_UNORM_S8_UINT,
        Tf::Depth32Float => DXGI_FORMAT_D32_FLOAT,
        Tf::Depth32FloatStencil8 => DXGI_FORMAT_D32_FLOAT_S8X24_UINT,
        Tf::NV12 => DXGI_FORMAT_NV12,
        Tf::P010 => DXGI_FORMAT_P010,
        Tf::Bc1RgbaUnorm => DXGI_FORMAT_BC1_UNORM,
        Tf::Bc1RgbaUnormSrgb => DXGI_FORMAT_BC1_UNORM_SRGB,
        Tf::Bc2RgbaUnorm => DXGI_FORMAT_BC2_UNORM,
        Tf::Bc2RgbaUnormSrgb => DXGI_FORMAT_BC2_UNORM_SRGB,
        Tf::Bc3RgbaUnorm => DXGI_FORMAT_BC3_UNORM,
        Tf::Bc3RgbaUnormSrgb => DXGI_FORMAT_BC3_UNORM_SRGB,
        Tf::Bc4RUnorm => DXGI_FORMAT_BC4_UNORM,
        Tf::Bc4RSnorm => DXGI_FORMAT_BC4_SNORM,
        Tf::Bc5RgUnorm => DXGI_FORMAT_BC5_UNORM,
        Tf::Bc5RgSnorm => DXGI_FORMAT_BC5_SNORM,
        Tf::Bc6hRgbUfloat => DXGI_FORMAT_BC6H_UF16,
        Tf::Bc6hRgbFloat => DXGI_FORMAT_BC6H_SF16,
        Tf::Bc7RgbaUnorm => DXGI_FORMAT_BC7_UNORM,
        Tf::Bc7RgbaUnormSrgb => DXGI_FORMAT_BC7_UNORM_SRGB,
        Tf::Etc2Rgb8Unorm
        | Tf::Etc2Rgb8UnormSrgb
        | Tf::Etc2Rgb8A1Unorm
        | Tf::Etc2Rgb8A1UnormSrgb
        | Tf::Etc2Rgba8Unorm
        | Tf::Etc2Rgba8UnormSrgb
        | Tf::EacR11Unorm
        | Tf::EacR11Snorm
        | Tf::EacRg11Unorm
        | Tf::EacRg11Snorm
        | Tf::Astc {
            block: _,
            channel: _,
        } => return None,
    })
}

pub fn map_texture_format(format: wgt::TextureFormat) -> Dxgi::Common::DXGI_FORMAT {
    match map_texture_format_failable(format) {
        Some(f) => f,
        None => unreachable!(),
    }
}

pub fn map_texture_format_for_resource(
    format: wgt::TextureFormat,
    usage: wgt::TextureUses,
    has_view_formats: bool,
    casting_fully_typed_format_supported: bool,
) -> Dxgi::Common::DXGI_FORMAT {
    use Dxgi::Common::*;
    use wgt::TextureFormat as Tf;

    if casting_fully_typed_format_supported {
        map_texture_format(format)

    // We might view this resource as srgb or non-srgb
    } else if has_view_formats {
        match format {
            Tf::Rgba8Unorm | Tf::Rgba8UnormSrgb => DXGI_FORMAT_R8G8B8A8_TYPELESS,
            Tf::Bgra8Unorm | Tf::Bgra8UnormSrgb => DXGI_FORMAT_B8G8R8A8_TYPELESS,
            Tf::Bc1RgbaUnorm | Tf::Bc1RgbaUnormSrgb => DXGI_FORMAT_BC1_TYPELESS,
            Tf::Bc2RgbaUnorm | Tf::Bc2RgbaUnormSrgb => DXGI_FORMAT_BC2_TYPELESS,
            Tf::Bc3RgbaUnorm | Tf::Bc3RgbaUnormSrgb => DXGI_FORMAT_BC3_TYPELESS,
            Tf::Bc7RgbaUnorm | Tf::Bc7RgbaUnormSrgb => DXGI_FORMAT_BC7_TYPELESS,
            format => map_texture_format(format),
        }

    // We might view this resource as SRV/UAV but also as DSV
    } else if format.is_depth_stencil_format()
        && usage.intersects(
            wgt::TextureUses::RESOURCE
                | wgt::TextureUses::STORAGE_READ_ONLY
                | wgt::TextureUses::STORAGE_WRITE_ONLY
                | wgt::TextureUses::STORAGE_READ_WRITE,
        )
    {
        match format {
            Tf::Depth16Unorm => DXGI_FORMAT_R16_TYPELESS,
            Tf::Depth32Float => DXGI_FORMAT_R32_TYPELESS,
            Tf::Depth32FloatStencil8 => DXGI_FORMAT_R32G8X24_TYPELESS,
            Tf::Stencil8 | Tf::Depth24Plus | Tf::Depth24PlusStencil8 => DXGI_FORMAT_R24G8_TYPELESS,
            _ => unreachable!(),
        }
    } else {
        map_texture_format(format)
    }
}

pub fn map_texture_usage_to_resource_flags(
    usage: wgt::TextureUses,
) -> Direct3D12::D3D12_RESOURCE_FLAGS {
    let mut flags = Direct3D12::D3D12_RESOURCE_FLAG_ALLOW_SIMULTANEOUS_ACCESS;

    if usage.contains(wgt::TextureUses::COLOR_TARGET) {
        flags |= Direct3D12::D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET;
    }
    if usage
        .intersects(wgt::TextureUses::DEPTH_STENCIL_READ | wgt::TextureUses::DEPTH_STENCIL_WRITE)
    {
        flags |= Direct3D12::D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL;
        if !usage.contains(wgt::TextureUses::RESOURCE) {
            flags |= Direct3D12::D3D12_RESOURCE_FLAG_DENY_SHADER_RESOURCE;
        }
    }
    if usage.intersects(
        wgt::TextureUses::STORAGE_READ_ONLY
            | wgt::TextureUses::STORAGE_WRITE_ONLY
            | wgt::TextureUses::STORAGE_READ_WRITE,
    ) {
        flags |= Direct3D12::D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS;
    }

    flags
}
