#[cfg(not(target_os = "linux"))]
mod opengl;

#[cfg(not(target_os = "linux"))]
pub use opengl::StageRenderer;

#[cfg(target_os = "linux")]
mod wgpu;

#[cfg(target_os = "linux")]
pub use wgpu::StageRenderer;
