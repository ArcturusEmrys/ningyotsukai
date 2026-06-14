#[derive(Debug, thiserror::Error)]
#[error("Could not initialize wgpu renderer: {0}")]
pub enum WgpuRendererError {
    InstanceError(#[from] wgpu_hal::InstanceError),
    CreateSurfaceError(#[from] wgpu::CreateSurfaceError),
    RequestAdapterError(#[from] wgpu::RequestAdapterError),
    RequestDeviceError(#[from] wgpu::RequestDeviceError),
    SurfaceError(#[from] ningyo_extensions::SurfaceError),

    #[error("Model rendering not initialized")]
    ModelRenderingNotInitialized,

    #[error("Size cannot be zero")]
    SizeCannotBeZero,

    #[error("Could not adopt texture as render target as it is missing required usages")]
    InvalidRenderTargetTexture,

    #[error("Viewport not yet initialized")]
    ViewportNotInitialized,

    #[error("The viewport ID {0} is not defined")]
    InvalidViewportId(usize),

    #[error("Model textures not of uniform size")]
    NonUniformModelTextureSizes,
}
