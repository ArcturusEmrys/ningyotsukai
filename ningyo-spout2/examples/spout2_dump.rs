use std::error::Error;

use ningyo_spout2::SenderRegistry;
use windows::Win32::Graphics::Direct3D9::{D3DFMT_A8R8G8B8, D3DFMT_X8R8G8B8};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_B8G8R8X8_UNORM, DXGI_FORMAT_R8G8B8A8_SNORM,
    DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, DXGI_FORMAT_R10G10B10A2_UNORM,
    DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_R16G16B16A16_SNORM, DXGI_FORMAT_R16G16B16A16_UNORM,
    DXGI_FORMAT_R32G32B32A32_FLOAT,
};

fn main() -> Result<(), Box<dyn Error>> {
    let registry = match SenderRegistry::new() {
        Ok(registry) => registry,
        Err(e) => {
            eprintln!("Error! {:?}", e);
            return Err(e.into());
        }
    };
    eprintln!("Registry capacity: {}", registry.capacity());
    eprintln!("Registry size: {}", registry.iter().count());

    for sender in registry.iter() {
        let sender_name = sender.to_string_lossy();
        eprintln!("Sender: {}", sender_name);

        let mut sender = match registry.open(sender) {
            Ok(sender) => sender,
            Err(e) => {
                eprintln!("  Error! {:?}", e);
                continue;
            }
        };

        if sender.has_event() {
            eprintln!("  Has frame event");
        }

        match sender.frame_count() {
            None => {}
            Some(Err(e)) => eprintln!("  Frame count: Error! {:?}", e),
            Some(Ok(count)) => eprintln!("  Frame count: {}", count),
        }

        let data = sender.data();

        eprintln!("  Size: {}x{}", data.width, data.height);
        //TODO: The legacy D3D formats shouldn't fit in the same enum.
        //Are we SURE Spout2 actually mixes these two enums?!
        if data.format == D3DFMT_A8R8G8B8.0 {
            eprintln!("  Format: D3DFMT_A8R8G8B8");
        } else if data.format == D3DFMT_X8R8G8B8.0 {
            eprintln!("  Format: D3DFMT_X8R8G8B8");
        } else if data.format as i32 == DXGI_FORMAT_B8G8R8X8_UNORM.0 {
            eprintln!("  Format: DXGI_FORMAT_B8G8R8X8_UNORM");
        } else if data.format as i32 == DXGI_FORMAT_B8G8R8A8_UNORM.0 {
            eprintln!("  Format: DXGI_FORMAT_B8G8R8A8_UNORM");
        } else if data.format as i32 == DXGI_FORMAT_R8G8B8A8_SNORM.0 {
            eprintln!("  Format: DXGI_FORMAT_R8G8B8A8_SNORM");
        } else if data.format as i32 == DXGI_FORMAT_R8G8B8A8_UNORM.0 {
            eprintln!("  Format: DXGI_FORMAT_R8G8B8A8_UNORM");
        } else if data.format as i32 == DXGI_FORMAT_R10G10B10A2_UNORM.0 {
            eprintln!("  Format: DXGI_FORMAT_R10G10B10A2_UNORM");
        } else if data.format as i32 == DXGI_FORMAT_R16G16B16A16_SNORM.0 {
            eprintln!("  Format: DXGI_FORMAT_R16G16B16A16_SNORM");
        } else if data.format as i32 == DXGI_FORMAT_R16G16B16A16_UNORM.0 {
            eprintln!("  Format: DXGI_FORMAT_R16G16B16A16_UNORM");
        } else if data.format as i32 == DXGI_FORMAT_R8G8B8A8_UNORM_SRGB.0 {
            eprintln!("  Format: DXGI_FORMAT_R8G8B8A8_UNORM_SRGB");
        } else if data.format as i32 == DXGI_FORMAT_R16G16B16A16_FLOAT.0 {
            eprintln!("  Format: DXGI_FORMAT_R16G16B16A16_FLOAT");
        } else if data.format as i32 == DXGI_FORMAT_R32G32B32A32_FLOAT.0 {
            eprintln!("  Format: DXGI_FORMAT_R32G32B32A32_FLOAT");
        }

        if let Some(desc) = data.description() {
            eprintln!("  Description: {}", desc.to_string_lossy());
        } else {
            eprintln!("  Description: (not a valid C string)");
        }

        eprintln!("  Handle: {:?}", data.share_handle());

        if data.is_cpu_sharing() {
            eprintln!("  Sender indicates CPU sharing in use");
        }

        if data.is_gldx_sharing() {
            eprintln!("  Sender indicates GL/DX sharing in use");
        }
    }

    Ok(())
}
