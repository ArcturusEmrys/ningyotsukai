mod wgpu;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "linux")]
pub mod vulkan;

pub use wgpu::{AdapterExt, DeviceExt, InstanceExt};
