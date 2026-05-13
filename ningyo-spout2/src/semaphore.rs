use std::ffi::CStr;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use crate::error::RegisterError;
use windows::Win32::Foundation::{HANDLE, WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0};
use windows::Win32::System::Threading::{
    CreateSemaphoreA, OpenSemaphoreW, ReleaseSemaphore, SEMAPHORE_MODIFY_STATE, WaitForSingleObject,
};
use windows_strings::{PCSTR, PWSTR};

/// A bare semaphore type that locks nothing and can be used as a generic
/// cross-process atomic counter.
pub struct BareSemaphore(OwnedHandle);

impl BareSemaphore {
    pub fn create(name: &CStr) -> Result<Self, RegisterError> {
        Ok(Self(unsafe {
            OwnedHandle::from_raw_handle(
                CreateSemaphoreA(
                    None,
                    1,
                    i32::MAX,
                    PCSTR::from_raw(name.as_ptr() as *const u8),
                )?
                .0,
            )
        }))
    }

    pub fn open(name: &str) -> Result<Self, RegisterError> {
        // For some reason, there's no A version of this function, so we have
        // to encode to UTF-16 (really, UCS-2) and send that.
        let wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();

        Ok(Self(unsafe {
            OwnedHandle::from_raw_handle(
                OpenSemaphoreW(
                    SEMAPHORE_MODIFY_STATE,
                    false,
                    PWSTR::from_raw(wide_name.as_ptr() as *mut u16),
                )?
                .0,
            )
        }))
    }

    pub fn increment(&mut self) -> Result<(), RegisterError> {
        let handle = HANDLE(self.0.as_raw_handle());
        match unsafe { WaitForSingleObject(handle, 0) } {
            WAIT_OBJECT_0 => unsafe { ReleaseSemaphore(handle, 2, None)? },
            WAIT_ABANDONED | WAIT_FAILED | _ => return Err(RegisterError::Poisoned),
        }

        Ok(())
    }

    pub fn count(&mut self) -> Result<i32, RegisterError> {
        let handle = HANDLE(self.0.as_raw_handle());
        match unsafe { WaitForSingleObject(handle, 0) } {
            WAIT_OBJECT_0 => {
                let mut framecount = 0;
                unsafe { ReleaseSemaphore(handle, 1, Some(&mut framecount))? }

                Ok(framecount)
            }
            WAIT_ABANDONED | WAIT_FAILED | _ => Err(RegisterError::Poisoned),
        }
    }
}
