mod error;
mod texture;
mod wgpu;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub mod dx12;

#[cfg(any(target_os = "windows", target_os = "linux"))]
pub mod vulkan;

pub use error::Error;
pub use texture::ExportableTexture;
pub use wgpu::{AdapterExt, ExtendedDevice, InstanceExt};

#[cfg(target_os = "linux")]
pub use linux::ExportedTexture;

pub mod prelude {
    pub use crate::wgpu::{AdapterExt, InstanceExt};
}
