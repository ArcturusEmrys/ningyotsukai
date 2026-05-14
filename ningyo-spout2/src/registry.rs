use std::collections::HashSet;
use std::ffi::{CStr, CString};
use std::os::raw::c_void;

use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueA};
use windows_result::Error as WindowsError;
use windows_strings::PCSTR;

use crate::error::RegisterError;
use crate::name::SenderName;
use crate::receiver::Receiver;
use crate::sender::Registration;
use crate::shm::{SharedCell, SharedSliceCell, SliceLockGuard};

const ACTIVE_SENDER_REGISTRY_NAME: &'static CStr = c"ActiveSenderName";

const SENDER_REGISTRY_NAME: &'static CStr = c"SpoutSenderNames";
const SENDER_REGISTRY_SUBKEY: &'static CStr = c"Software\\Leading Edge\\Spout";
const SENDER_REGISTRY_VALUENAME: &'static CStr = c"MaxSenders";

/// Read the sender set out into a HashSet, mutate it, and then write the set
/// back out.
///
/// This has the side effect of compacting the list down and removing any
/// invalid senders. It is the preferred way to mutate the sender registry.
///
/// `names` is intended to be a direct reference to Windows NT shared memory.
///
/// This function may return RegistryFull if the set has grown beyond the size
/// of the sender registry. If that has been returned, the memory in `names`
/// has not been modified.
pub fn with_sender_set<T, F: FnOnce(&mut HashSet<CString>) -> Result<T, RegisterError>>(
    names: &mut [SenderName],
    my_fn: F,
) -> Result<T, RegisterError> {
    let mut set = HashSet::new();

    for name in names.iter() {
        if let Some(name) = name.as_cstr() {
            if SharedCell::<()>::open(name).is_ok() {
                set.insert(name.to_owned());
            }
        }

        // NOTE: Spout2 null-terminates the list when writing but NOT when
        // reading. The 'correct' thing to do would be to stop reading entries
        // but we are maintaining the broken behavior for now.
        //
        // This appears to not be a problem in Spout2 because Spout2 will
        // attempt to open registered files and remove senders whose files
        // fail to open.
    }

    let out = my_fn(&mut set)?;

    if set.len() > names.len() {
        return Err(RegisterError::RegistryFull);
    }

    for (src, dest) in set
        .iter()
        .filter_map(|s| SenderName::try_from_cstr(Some(&s)))
        .chain(std::iter::once(SenderName::invalid()))
        .zip(names)
    {
        *dest = src;
    }

    Ok(out)
}

pub struct SenderRegistry {
    senders: SharedSliceCell<SenderName>,
    active: SharedCell<SenderName>,
}

impl SenderRegistry {
    pub fn new() -> Result<Self, WindowsError> {
        // Default size of the senders array.
        // Due to the fact that this is shared across the entire window
        // session, we have to follow the MaxSenders registry key to avoid
        // giving the wrong amount of memory to C, since the C version doesn't
        // call NtQuerySection to get the appropriate size.
        let mut senders: u32 = 64;
        let mut data: u32 = 0;
        let mut size: u32 = size_of::<u32>() as u32;

        let result = unsafe {
            RegGetValueA(
                HKEY_CURRENT_USER,
                PCSTR::from_raw(SENDER_REGISTRY_SUBKEY.as_ptr() as *const u8),
                PCSTR::from_raw(SENDER_REGISTRY_VALUENAME.as_ptr() as *const u8),
                RRF_RT_REG_DWORD,
                None,
                Some(&mut data as *mut u32 as *mut c_void),
                Some(&mut size as *mut u32),
            )
        };

        if result.is_ok() {
            senders = data;
        }

        Ok(SenderRegistry {
            senders: SharedSliceCell::create(
                SENDER_REGISTRY_NAME,
                size_of::<SenderName>() * senders as usize,
            )?,
            active: SharedCell::create(ACTIVE_SENDER_REGISTRY_NAME)?,
        })
    }

    pub fn register(&self, name: &str) -> Result<Registration, RegisterError> {
        let mut cstr_name = CString::new(name)?;
        let data = SharedCell::create(&cstr_name)?;

        with_sender_set(&mut *self.senders.lock().unwrap(), |set| {
            //Attempt to register a fallback name.
            let mut fallback_count = 0;
            while set.contains(&cstr_name) {
                fallback_count += 1;
                cstr_name = CString::new(format!("{}_{}", name, fallback_count))?;
            }

            set.insert(cstr_name.clone());
            Ok(())
        })?;

        Ok(Registration {
            senders: self.senders.clone(),
            active: self.active.clone(),
            data,
            name: cstr_name,
            event: None,
            frame_count: None,
        })
    }

    pub fn open(&self, name: &CStr) -> Result<Receiver, RegisterError> {
        //TODO: This should check if the name is still in the receiver set and
        //error out if not
        Receiver::new(name.into())
    }

    pub fn iter(&self) -> impl Iterator<Item = &CStr> {
        SenderRegistryIterator::new(self.senders.lock().unwrap())
    }

    pub fn capacity(&self) -> usize {
        self.senders.lock().unwrap().len()
    }
}

pub struct SenderRegistryIterator<'a> {
    _lock_guard: SliceLockGuard<'a, SenderName>,
    ptr: *const SenderName,
    end: *const SenderName,
}

impl<'a> SenderRegistryIterator<'a> {
    fn new(lock_guard: SliceLockGuard<'a, SenderName>) -> Self {
        let ptr = lock_guard.as_ptr();
        //TODO: I'm assuming we will never get a slice with an insane length.
        let end = unsafe { lock_guard.as_ptr().add(lock_guard.len()) };
        Self {
            _lock_guard: lock_guard,
            ptr,
            end,
        }
    }
}

impl<'a> Iterator for SenderRegistryIterator<'a> {
    type Item = &'a CStr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.ptr < self.end {
            // SAFETY: We just bounds-checked the pointer
            // SAFETY: The pointer comes from a valid Rust slice
            // SAFETY: The pointer is valid for as long as we hold the lock
            // guard
            let next = unsafe { self.ptr.add(1) };
            let cur_ref = unsafe { &*self.ptr };

            self.ptr = next;

            Some(cur_ref.as_cstr()?)
        } else {
            None
        }
    }
}
