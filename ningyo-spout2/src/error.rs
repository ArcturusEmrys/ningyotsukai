use std::ffi::NulError;

use windows_result::Error as WindowsError;

#[derive(Debug, thiserror::Error)]
pub enum RegisterError {
    #[error("{0}")]
    NulError(#[from] NulError),

    #[error("{0}")]
    WindowsError(#[from] WindowsError),

    #[error("The sender registry is full. No new senders can be registered until one terminates.")]
    RegistryFull,

    #[error("The given object has been abandoned by its prior holder.")]
    Poisoned,
}
