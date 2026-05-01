use std::ffi::CString;
use std::os::raw::c_void;

use windows::Win32::Foundation::HANDLE;

use crate::name::SenderName;
use crate::registry::with_sender_set;
use crate::shm::{SharedCell, SharedSliceCell};

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Zeroable, bytemuck::Pod)]
pub struct SharedTextureInfo {
    /// A DirectX 11 share handle.
    ///
    /// Ultimately, what happens with Spout receivers is that they will take
    /// this handle and create an ID3D11Texture2D from it using
    /// OpenSharedResource. Senders should ensure their texture is created with
    /// the D3D11_RESOURCE_MISC_SHARED flag and shared with the
    /// DXGI_SHARED_RESOURCE_READ and DXGI_SHARED_RESOURCE_WRITE flags.
    ///
    /// There's code in Spout to use SHARED_NTHANDLE and/or SHARED_KEYEDMUTEX,
    /// but it does not appear to be used. Spout always seems to create
    /// textures only using the legacy shared flag.
    ///
    /// This value must be coerced into a HANDLE by sign extension.
    ///
    /// Senders that wish to use other graphics APIs (OpenGL, D3D9, D3D12, or
    /// Vulkan) must copy their rendered output into a D3D11 texture and
    /// present that to Spout. The vast majority of code in Spout2 is
    /// convenience wrappers specifically to manage the hell that is cross-API
    /// and cross-process texture sharing. For the equivalent code in Rust, see
    /// the `ningyo-texshare` crate.
    share_handle: i32,
    width: u32,
    height: u32,

    /// A DirectX 11 format enum value.
    ///
    /// The following texture formats are compatible with Spout2's texture
    /// sharing logic:
    ///
    ///  * D3DFMT_A8R8G8B8
    ///  * D3DFMT_X8R8G8B8
    ///  * DXGI_FORMAT_B8G8R8X8_UNORM
    ///  * DXGI_FORMAT_B8G8R8A8_UNORM
    ///  * DXGI_FORMAT_R8G8B8A8_SNORM
    ///  * DXGI_FORMAT_R8G8B8A8_UNORM
    ///  * DXGI_FORMAT_R10G10B10A2_UNORM
    ///  * DXGI_FORMAT_R16G16B16A16_SNORM
    ///  * DXGI_FORMAT_R16G16B16A16_UNORM
    ///  * DXGI_FORMAT_R8G8B8A8_UNORM_SRGB
    ///  * DXGI_FORMAT_R16G16B16A16_FLOAT
    ///  * DXGI_FORMAT_R32G32B32A32_FLOAT
    format: u32,

    /// Unused as per Spout2 source.
    _usage: u32,

    /// An arbitrary description field.
    ///
    /// Spout2 typically puts the current process path here.
    description: SenderName,

    /// Bit flags that indicate how the sender is providing their texture.
    ///
    /// Bit 31: CPU sharing
    /// Bit 30: GLDX sharing
    ///
    /// Spout2 source claims this is informative only.
    partner_id: u32,
}

impl SharedTextureInfo {
    pub fn share_handle(&self) -> HANDLE {
        HANDLE(self.share_handle as isize as *mut c_void)
    }

    pub fn set_share_handle(&mut self, handle: HANDLE) {
        self.share_handle = handle.0 as isize as i32;
    }
}

/// An object that represents the registration of a Spout2 sender.
///
/// Registration can be cancelled by dropping the registration object. To
/// maintain your sender registration, hold the object until it is no longer
/// needed.
pub struct Registration {
    pub(crate) senders: SharedSliceCell<SenderName>,
    pub(crate) active: SharedCell<SenderName>,
    pub(crate) data: SharedCell<SharedTextureInfo>,
    pub(crate) name: CString,
}

impl Registration {
    pub fn publish_dx11_texture(&self, width: u32, height: u32, format: u32, share_handle: HANDLE) {
        let mut data = self.data.lock().unwrap();

        data.width = width;
        data.height = height;
        data.format = format;
        data.set_share_handle(share_handle);
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        let mut data = self.senders.lock().unwrap();
        let mut remaining_sender = None;

        // NOTE: This cannot actually panic as the set is only ever shrunk.
        with_sender_set(&mut *data, |set| {
            set.remove(&self.name);
            remaining_sender = set.iter().next().cloned();

            Ok(())
        })
        .unwrap();

        // If the current sender was active, deactivate it (making another
        // sender active)
        let mut active = self.active.lock().unwrap();
        if Some(SenderName(active.0)) == SenderName::try_from_cstr(Some(&self.name)) {
            *active = SenderName::try_from_cstr(remaining_sender.as_deref())
                .unwrap_or_else(SenderName::invalid);
        }
    }
}
