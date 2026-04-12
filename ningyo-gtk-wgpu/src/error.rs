use ash::vk::Result as VkResult;
use thiserror::Error;
use wgpu_hal::DeviceError;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Wgpu (HAL) error: {0}")]
    HalDeviceError(#[from] DeviceError),

    #[error("Vulkan API error: {0}")]
    VulkanError(#[from] VkResult),

    #[error("API of object does not match capabilities of called function")]
    WrongAPIError,

    #[error(
        "An extension necessary to process the request is missing. Please ensure your instance and device were both created with the appropriate _with_extensions method."
    )]
    MissingExtension,

    #[error("The texture cannot be exported as it is already external and opaque to the device")]
    OpaqueExport,

    #[error("The file descriptor provided or obtained was invalid.")]
    InvalidFd,

    #[error(
        "The texture format cannot be represented in the target API or as an exportable texture."
    )]
    InvalidFormat,

    #[error("No valid memory types found to store exportable texture.")]
    NoValidMemoryType,

    #[error("GDK yielded the following error: {0}")]
    GdkError(#[from] glib::Error),
}
