use windows::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, ERROR_INSUFFICIENT_BUFFER, ERROR_INTERNAL_ERROR, ERROR_INVALID_PARAMETER,
    ERROR_NOT_FOUND, GetLastError, HANDLE, INVALID_HANDLE_VALUE, NTSTATUS, STATUS_SUCCESS,
    WAIT_ABANDONED, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::System::Memory::{
    CreateFileMappingA, FILE_MAP_ALL_ACCESS, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
    OpenFileMappingA, PAGE_READWRITE, UnmapViewOfFile,
};
use windows::Win32::System::Threading::{
    CreateMutexA, INFINITE, ReleaseMutex, WaitForSingleObject,
};
use windows_result::Error as WindowsError;
use windows_strings::PCSTR;

use ntapi::ntmmapi::{NtQuerySection, SECTION_BASIC_INFORMATION, SectionBasicInformation}; //chthulu ftaghn

use bytemuck::AnyBitPattern;

use core::slice;
use std::ffi::{CStr, CString, c_void};
use std::marker::PhantomData;
use std::mem::{size_of, zeroed};
use std::ops::{Deref, DerefMut};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::ptr::NonNull;
use std::sync::Arc;

/// Represents a bare pointer to a Windows file mapping.
#[derive(Debug)]
pub struct Mapping<T>(*mut T);

impl<T> Mapping<T> {
    fn from_file_handle(file: HANDLE) -> Arc<Self> {
        let map = unsafe { MapViewOfFile(file, FILE_MAP_ALL_ACCESS, 0, 0, 0).Value };

        Arc::new(Self(map as *mut T))
    }

    const fn get(&self) -> *mut T {
        self.0
    }
}

impl<T> Drop for Mapping<T> {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS {
                    Value: self.0 as *mut c_void,
                })
                .unwrap();
            }
        }
    }
}

/// An UnsafeCell-like abstraction on top of NT shared memory handles and
/// mutexes.
#[derive(Debug)]
pub struct UnsafeSharedCell<T> {
    file: OwnedHandle,
    map: Arc<Mapping<T>>,
    size: usize,
}

impl<T> UnsafeSharedCell<T> {
    /// Create a shared memory object with a specific name.
    ///
    /// The name passed to create uniquely identifies a specific memory region
    /// within the current global memory handle namespace. Thus, this can be
    /// used to provide inter-process communication.
    ///
    /// The size parameter is in units of bytes. If the shared memory object
    /// has already been created, it may be of a different size. If the size
    /// is too small for the given type, creation of the `UnsafeSharedCell`
    /// will fail with either `ERROR_INVALID_PARAMETER` (if your parameter is
    /// too low) or `ERROR_INSUFFICIENT_BUFFER` (if the file is too small for
    /// the type). It is not possible to resize an already created shared
    /// mapping.
    ///
    /// The trait bound on this function is `AnyBitPattern` for the following
    /// reasons:
    ///
    /// 1. Newly created shared files are initialized to all zeroes
    /// 2. NT shared handles have no Rust-level type restrictions. Opening two
    /// files with different types will succeed and is morally equivalent to a
    /// `std::mem::transmute` of the pointed-to data.
    /// 3. NT shared handles can be opened and mapped multiple times in the
    /// same process. Doing so will produce two pointers to the same data,
    /// which is morally equivalent to moving the data.
    pub fn create(name: &CStr, mut size: usize) -> Result<Self, WindowsError>
    where
        T: AnyBitPattern + Unpin,
    {
        if size < size_of::<T>() {
            return Err(WindowsError::from_hresult(ERROR_INVALID_PARAMETER.into()));
        }

        let file = unsafe {
            CreateFileMappingA(
                INVALID_HANDLE_VALUE,
                None,
                PAGE_READWRITE,
                (size >> 32) as u32,
                (size & 0xFFFFFFFF) as u32,
                PCSTR::from_raw(name.as_ptr() as *const u8),
            )?
        };

        if file.is_invalid() {
            return Err(WindowsError::from_hresult(ERROR_INTERNAL_ERROR.into()));
        }

        let file_error = unsafe { GetLastError() };

        if file_error == ERROR_ALREADY_EXISTS {
            // Oh no. We got handed someone else's memory.
            //
            // The problem is, we have no idea what size it is now, it could
            // be tiny. In fact, the original Spout2 implementation already
            // has this problem; they try to work around it with registry keys
            // which I'm not going to trust.
            //
            // Instead, we're going to rely on private APIs!

            let mut sbi: SECTION_BASIC_INFORMATION = unsafe { zeroed() };
            let mut return_length = 0;
            let status = NTSTATUS(unsafe {
                NtQuerySection(
                    std::mem::transmute(file.0), // why does every crate redefine *void
                    SectionBasicInformation,
                    &mut sbi as *mut _ as *mut _,
                    size_of::<SECTION_BASIC_INFORMATION>(),
                    &mut return_length,
                )
            });

            if status == STATUS_SUCCESS {
                size = unsafe { *sbi.MaximumSize.QuadPart() as usize };
            } else {
                return Err(WindowsError::from_hresult(status.into()));
            }
        }

        unsafe { Self::from_raw_file_mapping(file, size) }
    }

    /// Open a shared memory object with a specific name.
    ///
    /// The name passed to create uniquely identifies a specific memory region
    /// within the current global memory handle namespace. Thus, this can be
    /// used to provide inter-process communication. The file must already
    /// have been created by another process - calling this function will NOT
    /// create the file.
    ///
    /// If the shared memory object has already been created, it may be of a
    /// different size. If the size is too small for the given type, creation
    /// of the `UnsafeSharedCell` will fail with `ERROR_INSUFFICIENT_BUFFER`.
    /// It is not possible to resize an already created shared mapping.
    ///
    /// The trait bound on this function is `AnyBitPattern` for the following
    /// reasons:
    ///
    /// 1. Newly created shared files are initialized to all zeroes
    /// 2. NT shared handles have no Rust-level type restrictions. Opening two
    /// files with different types will succeed and is morally equivalent to a
    /// `std::mem::transmute` of the pointed-to data.
    /// 3. NT shared handles can be opened and mapped multiple times in the
    /// same process. Doing so will produce two pointers to the same data,
    /// which is morally equivalent to moving the data.
    pub fn open(name: &CStr) -> Result<Self, WindowsError>
    where
        T: AnyBitPattern + Unpin,
    {
        let file = unsafe {
            OpenFileMappingA(
                FILE_MAP_ALL_ACCESS.0,
                false,
                PCSTR::from_raw(name.as_ptr() as *const u8),
            )?
        };

        if file.is_invalid() {
            return Err(WindowsError::from_hresult(ERROR_NOT_FOUND.into()));
        }

        // Oh look, we're calling NTAPI functions again.
        // See `create` as for why.
        let mut sbi: SECTION_BASIC_INFORMATION = unsafe { zeroed() };
        let mut return_length = 0;
        let status = NTSTATUS(unsafe {
            NtQuerySection(
                std::mem::transmute(file.0), // why does every crate redefine *void
                SectionBasicInformation,
                &mut sbi as *mut _ as *mut _,
                size_of::<SECTION_BASIC_INFORMATION>(),
                &mut return_length,
            )
        });

        if status == STATUS_SUCCESS {
            let size = unsafe { *sbi.MaximumSize.QuadPart() as usize };

            unsafe { Self::from_raw_file_mapping(file, size) }
        } else {
            Err(WindowsError::from_hresult(status.into()))
        }
    }

    /// Create an UnsafeSharedCell from a raw file mapping and the file's
    /// size.
    ///
    /// SAFETY: The size parameter better match the file.
    unsafe fn from_raw_file_mapping(file: HANDLE, size: usize) -> Result<Self, WindowsError>
    where
        T: AnyBitPattern + Unpin,
    {
        if size < size_of::<T>() {
            return Err(WindowsError::from_hresult(ERROR_INSUFFICIENT_BUFFER.into()));
        }

        let map = Mapping::from_file_handle(file);

        Ok(Self {
            file: unsafe { OwnedHandle::from_raw_handle(file.0) },
            map,
            size,
        })
    }

    /// Return a pointer to the shared memory region as type T.
    ///
    /// No guarantees are made regarding the contents of the memory in the
    /// region.
    pub fn get(&self) -> *mut T {
        self.map.get()
    }

    /// Retrieve the size of the shared cell.
    pub fn size(&self) -> usize {
        self.size
    }
}

impl<T> Clone for UnsafeSharedCell<T> {
    fn clone(&self) -> Self {
        UnsafeSharedCell {
            file: self.file.try_clone().unwrap(),
            map: self.map.clone(),
            size: self.size,
        }
    }
}

#[derive(Debug)]
pub struct SharedCell<T> {
    inner: UnsafeSharedCell<T>,
    mutex: OwnedHandle,
}

impl<T> SharedCell<T> {
    /// Create a shared memory object with a specific name.
    ///
    /// The name passed to create uniquely identifies a specific memory region
    /// within the current global memory handle namespace. Thus, this can be
    /// used to provide inter-process communication.
    ///
    /// The file will be created with the size of T. If the shared memory
    /// object has already been created, it must be large enough to store that
    /// amount of data, or this function will fail.
    ///
    /// Additionally, a mutex will be created with name ending in `_mutex`. As
    /// a result, this function may fail if called with a name that ends in
    /// `_mutex`.
    pub fn create(name: &CStr) -> Result<Self, WindowsError>
    where
        T: AnyBitPattern + Unpin,
    {
        // SAFETY: I have no clue what a zero allocation would do, so if safe
        // Rust tries to make an array of ZSTs, just allocate one byte.
        let mut bytes_size = size_of::<T>();
        if bytes_size == 0 {
            bytes_size = 1;
        }

        let inner = UnsafeSharedCell::create(name, bytes_size)?;

        let mut mutex_name = name.to_bytes().to_vec();
        mutex_name.extend_from_slice(c"_mutex".to_bytes_with_nul());
        let mutex_name = CString::from_vec_with_nul(mutex_name).unwrap();
        let mutex = unsafe {
            CreateMutexA(
                None,
                false,
                PCSTR::from_raw(mutex_name.as_ptr() as *const u8),
            )?
        };

        if mutex.is_invalid() {
            return Err(WindowsError::from_hresult(ERROR_INTERNAL_ERROR.into()));
        }

        Ok(Self {
            inner,
            mutex: unsafe { OwnedHandle::from_raw_handle(mutex.0) },
        })
    }

    /// Open a shared memory object with a specific name.
    ///
    /// The name passed to create uniquely identifies a specific memory region
    /// within the current global memory handle namespace. Thus, this can be
    /// used to provide inter-process communication. The NT memory file must
    /// already exist.
    ///
    /// If the open file is not large enough to store a value of type T, then
    /// opening the file will fail.
    ///
    /// Additionally, a mutex will be created with name ending in `_mutex`. As
    /// a result, this function may fail if called with a name that ends in
    /// `_mutex`.
    pub fn open(name: &CStr) -> Result<Self, WindowsError>
    where
        T: AnyBitPattern + Unpin,
    {
        let inner = UnsafeSharedCell::open(name)?;

        let mut mutex_name = name.to_bytes().to_vec();
        mutex_name.extend_from_slice(c"_mutex".to_bytes_with_nul());
        let mutex_name = CString::from_vec_with_nul(mutex_name).unwrap();
        let mutex = unsafe {
            CreateMutexA(
                None,
                false,
                PCSTR::from_raw(mutex_name.as_ptr() as *const u8),
            )?
        };

        if mutex.is_invalid() {
            return Err(WindowsError::from_hresult(ERROR_INTERNAL_ERROR.into()));
        }

        Ok(Self {
            inner,
            mutex: unsafe { OwnedHandle::from_raw_handle(mutex.0) },
        })
    }

    /// Lock the shared memory region and return type T.
    pub fn try_lock<'a>(
        &'a self,
        timeout_ms: u32,
    ) -> Result<LockGuard<'a, T>, TryLockError<LockGuard<'a, T>>> {
        let result = unsafe { WaitForSingleObject(HANDLE(self.mutex.as_raw_handle()), timeout_ms) };

        match result {
            //TODO: PoisonError
            WAIT_ABANDONED => Err(TryLockError::Poisoned(PoisonError::new(LockGuard {
                shm: self,
                mem: unsafe { NonNull::new_unchecked(self.inner.map.get()) },
                marker: PhantomData::default(),
            }))),
            WAIT_TIMEOUT => Err(TryLockError::Timeout),
            WAIT_FAILED => Err(WindowsError::from_thread().into()),
            WAIT_OBJECT_0 => Ok(LockGuard {
                shm: self,
                mem: unsafe { NonNull::new_unchecked(self.inner.map.get()) },
                marker: PhantomData::default(),
            }),
            _ => unreachable!(),
        }
    }

    pub fn lock<'a>(&'a self) -> Result<LockGuard<'a, T>, TryLockError<LockGuard<'a, T>>> {
        self.try_lock(INFINITE)
    }
}

impl<T> Clone for SharedCell<T> {
    fn clone(&self) -> Self {
        SharedCell {
            inner: self.inner.clone(),
            mutex: self.mutex.try_clone().unwrap(),
        }
    }
}

/// LockGuard for a shared memory region.
///
/// It is unsound to construct a lock guard to a type T that is not
/// transmutable from any prior type T' that was written into the shared
/// memory prior. In practice, you should use the same T each time.
///
/// To enforce this invariant, there is no safe function to get a lock guard
/// for a different type than what the shared memory is for.
#[derive(Debug)]
pub struct LockGuard<'a, T> {
    shm: &'a SharedCell<T>,
    mem: NonNull<T>,
    marker: PhantomData<&'a mut T>,
}

impl<'a, T> Deref for LockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: A LockGuard can only be acquired after the associated mutex has been locked.
        unsafe { &*self.mem.as_ptr() }
    }
}

impl<'a, T> DerefMut for LockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: A LockGuard can only be acquired after the associated mutex has been locked.
        unsafe { &mut *self.mem.as_mut() }
    }
}

impl<'a, T> Drop for LockGuard<'a, T> {
    fn drop(&mut self) {
        unsafe { ReleaseMutex(HANDLE(self.shm.mutex.as_raw_handle())).unwrap() }
    }
}

#[derive(Debug)]
pub struct SharedSliceCell<T> {
    inner: UnsafeSharedCell<T>,
    mutex: OwnedHandle,
}

impl<T> SharedSliceCell<T> {
    /// Create a shared memory object with a specific name.
    ///
    /// The name passed to create uniquely identifies a specific memory region
    /// within the current global memory handle namespace. Thus, this can be
    /// used to provide inter-process communication.
    ///
    /// The size parameter is in units of the associated type T. If the shared
    /// memory object has already been created, it may be of a different size.
    ///
    /// Additionally, a mutex will be created with name ending in `_mutex`. As
    /// a result, this function may fail if called with a name that ends in
    /// `_mutex`.
    pub fn create(name: &CStr, size: usize) -> Result<Self, WindowsError>
    where
        T: AnyBitPattern + Unpin,
    {
        // SAFETY: Prevent allocationg more memory than can exist on this
        // platform
        if size_of::<T>() > 0 && usize::MAX / size_of::<T>() < size {
            return Err(WindowsError::from_hresult(ERROR_INVALID_PARAMETER.into()));
        }

        // SAFETY: I have no clue what a zero allocation would do, so if safe
        // Rust tries to make an array of ZSTs, just allocate one byte.
        let mut bytes_size = size * size_of::<T>();
        if bytes_size == 0 {
            bytes_size = 1;
        }

        // TODO: It's technically valid to open a SliceCell to a too-small file.
        // It should return an empty slice instead of failing.
        let inner = UnsafeSharedCell::create(name, bytes_size)?;

        let mut mutex_name = name.to_bytes().to_vec();
        mutex_name.extend_from_slice(c"_mutex".to_bytes_with_nul());
        let mutex_name = CString::from_vec_with_nul(mutex_name).unwrap();
        let mutex = unsafe {
            CreateMutexA(
                None,
                false,
                PCSTR::from_raw(mutex_name.as_ptr() as *const u8),
            )?
        };

        if mutex.is_invalid() {
            return Err(WindowsError::from_hresult(ERROR_INTERNAL_ERROR.into()));
        }

        Ok(Self {
            inner,
            mutex: unsafe { OwnedHandle::from_raw_handle(mutex.0) },
        })
    }

    /// Open a shared memory object with a specific name.
    ///
    /// The name passed to create uniquely identifies a specific memory region
    /// within the current global memory handle namespace. Thus, this can be
    /// used to provide inter-process communication. The NT memory file must
    /// already exist.
    ///
    /// The size parameter is in units of the associated type T. If the shared
    /// memory object has already been created, it may be of a different size.
    ///
    /// Additionally, a mutex will be created with name ending in `_mutex`. As
    /// a result, this function may fail if called with a name that ends in
    /// `_mutex`.
    pub fn open(name: &CStr) -> Result<Self, WindowsError>
    where
        T: AnyBitPattern + Unpin,
    {
        let inner = UnsafeSharedCell::open(name)?;

        let mut mutex_name = name.to_bytes().to_vec();
        mutex_name.extend_from_slice(c"_mutex".to_bytes_with_nul());
        let mutex_name = CString::from_vec_with_nul(mutex_name).unwrap();
        let mutex = unsafe {
            CreateMutexA(
                None,
                false,
                PCSTR::from_raw(mutex_name.as_ptr() as *const u8),
            )?
        };

        if mutex.is_invalid() {
            return Err(WindowsError::from_hresult(ERROR_INTERNAL_ERROR.into()));
        }

        Ok(Self {
            inner,
            mutex: unsafe { OwnedHandle::from_raw_handle(mutex.0) },
        })
    }

    /// Lock the shared memory region and return type T.
    pub fn try_lock<'a>(
        &'a self,
        timeout_ms: u32,
    ) -> Result<SliceLockGuard<'a, T>, TryLockError<SliceLockGuard<'a, T>>> {
        let result = unsafe { WaitForSingleObject(HANDLE(self.mutex.as_raw_handle()), timeout_ms) };

        match result {
            //TODO: PoisonError
            WAIT_ABANDONED => Err(TryLockError::Poisoned(PoisonError::new(SliceLockGuard {
                shm: self,
                mem: unsafe { NonNull::new_unchecked(self.inner.get()) },
                marker: PhantomData::default(),
            }))),
            WAIT_TIMEOUT => Err(TryLockError::Timeout),
            WAIT_FAILED => Err(WindowsError::from_thread().into()),
            WAIT_OBJECT_0 => Ok(SliceLockGuard {
                shm: self,
                mem: unsafe { NonNull::new_unchecked(self.inner.get()) },
                marker: PhantomData::default(),
            }),
            _ => unreachable!(),
        }
    }

    pub fn lock<'a>(
        &'a self,
    ) -> Result<SliceLockGuard<'a, T>, TryLockError<SliceLockGuard<'a, T>>> {
        self.try_lock(INFINITE)
    }
}

impl<T> Clone for SharedSliceCell<T> {
    fn clone(&self) -> Self {
        SharedSliceCell {
            inner: self.inner.clone(),
            mutex: self.mutex.try_clone().unwrap(),
        }
    }
}

/// An error returned from a failed mutex locking attempt.
#[derive(Debug, thiserror::Error)]
pub enum TryLockError<Guard> {
    /// The lock could not be acquired because another thread or process
    /// abandoned the associated memory.
    Poisoned(#[from] PoisonError<Guard>),

    /// The lock could not be acquired within the timeout specified.
    Timeout,

    /// An operating system error occurred while attempting to lock the
    /// shared memory.
    OsError(#[from] WindowsError),
}

#[derive(Debug, thiserror::Error)]
pub struct PoisonError<Guard> {
    data: Guard,
}

impl<Guard> PoisonError<Guard> {
    pub fn new(data: Guard) -> PoisonError<Guard> {
        PoisonError { data }
    }

    pub fn into_inner(self) -> Guard {
        self.data
    }

    pub fn get_ref(&self) -> &Guard {
        &self.data
    }

    pub fn get_mut(&mut self) -> &mut Guard {
        &mut self.data
    }
}

/// LockGuard for a shared memory region.
///
/// It is unsound to construct a lock guard to a type T that is not
/// transmutable from any prior type T' that was written into the shared
/// memory prior. In practice, you should use the same T each time.
///
/// To enforce this invariant, there is no safe function to get a lock guard
/// for a different type than what the shared memory is for.
#[derive(Debug)]
pub struct SliceLockGuard<'a, T> {
    shm: &'a SharedSliceCell<T>,
    mem: NonNull<T>,
    marker: PhantomData<&'a mut T>,
}

impl<'a, T> Deref for SliceLockGuard<'a, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        // SAFETY: A LockGuard can only be acquired after the associated mutex has been locked.
        unsafe { slice::from_raw_parts(self.mem.as_ptr(), self.shm.inner.size() / size_of::<T>()) }
    }
}

impl<'a, T> DerefMut for SliceLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: A LockGuard can only be acquired after the associated mutex has been locked.
        unsafe {
            slice::from_raw_parts_mut(self.mem.as_mut(), self.shm.inner.size() / size_of::<T>())
        }
    }
}

impl<'a, T> Drop for SliceLockGuard<'a, T> {
    fn drop(&mut self) {
        unsafe { ReleaseMutex(HANDLE(self.shm.mutex.as_raw_handle())).unwrap() }
    }
}
