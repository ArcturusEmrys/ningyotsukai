mod opengl;
mod wgpu;

#[cfg(any(target_os = "linux", target_os = "windows"))]
pub use wgpu::StageRenderer;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub use opengl::StageRenderer;
