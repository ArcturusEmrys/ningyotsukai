mod wgpu;
mod error;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "linux")]
pub mod vulkan;

pub use wgpu::{AdapterExt, DeviceExt, InstanceExt};
pub use error::Error;

#[cfg(target_os="linux")]
pub use linux::ExportedTexture;

pub mod prelude {
    pub use crate::wgpu::{AdapterExt, DeviceExt, InstanceExt};
}