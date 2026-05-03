use std::ffi::CStr;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use windows::Win32::Foundation::{
    HANDLE, WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::System::Threading::{
    CreateEventA, EVENT_ALL_ACCESS, OpenEventA, WaitForSingleObject,
};
use windows_result::Error as WindowsError;
use windows_strings::PCSTR;

use crate::error::RegisterError;

pub struct BareEvent(OwnedHandle);

impl BareEvent {
    pub fn create(name: &CStr) -> Result<Self, RegisterError> {
        Ok(Self(unsafe {
            OwnedHandle::from_raw_handle(
                CreateEventA(
                    None,
                    false,
                    false,
                    PCSTR::from_raw(name.as_ptr() as *const u8),
                )?
                .0,
            )
        }))
    }

    pub fn open(name: &CStr) -> Result<Self, RegisterError> {
        Ok(Self(unsafe {
            OwnedHandle::from_raw_handle(
                OpenEventA(
                    EVENT_ALL_ACCESS,
                    true,
                    PCSTR::from_raw(name.as_ptr() as *const u8),
                )?
                .0,
            )
        }))
    }

    pub fn wait(&mut self, timeout: u32) -> Result<(), RegisterError> {
        let handle = HANDLE(self.0.as_raw_handle());
        match unsafe { WaitForSingleObject(handle, timeout) } {
            WAIT_OBJECT_0 => Ok(()),
            WAIT_ABANDONED => Err(RegisterError::Poisoned),
            WAIT_TIMEOUT => Err(RegisterError::TimedOut),
            WAIT_FAILED | _ => Err(WindowsError::from_thread().into()),
        }
    }
}
