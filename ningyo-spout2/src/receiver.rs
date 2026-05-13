use std::ffi::CString;

use crate::event::BareEvent;
use crate::name::SenderName;
use crate::semaphore::BareSemaphore;
use crate::sender::SharedTextureInfo;
use crate::shm::{SharedCell, SharedSliceCell};
use crate::{RegisterError, Registration};

/// An object that represents a connection to an existing Spout2 sender in
/// another process.
pub struct Receiver {
    senders: SharedSliceCell<SenderName>,
    active: SharedCell<SenderName>,
    data: SharedCell<SharedTextureInfo>,
    name: CString,
    event: Option<BareEvent>,
    frame_count: Option<BareSemaphore>,
}

impl Receiver {
    pub fn new(
        senders: SharedSliceCell<SenderName>,
        active: SharedCell<SenderName>,
        name: CString,
    ) -> Result<Self, RegisterError> {
        let data = SharedCell::open(&name)?;

        let mut event_name = name.clone().into_bytes();
        event_name.extend_from_slice(Registration::EVENT_SUFFIX.to_bytes_with_nul());
        let event_name = CString::from_vec_with_nul(event_name).unwrap();
        let event = BareEvent::open(&event_name).ok();

        let mut count_name = name.clone().into_bytes();
        count_name.extend_from_slice(Registration::SEMAPHORE_SUFFIX.to_bytes_with_nul());
        let count_name = CString::from_vec_with_nul(count_name).unwrap();
        let count_name_utf = count_name.into_string().unwrap();
        let frame_count = BareSemaphore::open(&count_name_utf).ok();

        Ok(Self {
            senders,
            active,
            data,
            name,
            event,
            frame_count,
        })
    }

    pub fn frame_count(&mut self) -> Option<Result<i32, RegisterError>> {
        if let Some(frame_count) = &mut self.frame_count {
            Some(frame_count.count())
        } else {
            None
        }
    }

    pub fn has_event(&self) -> bool {
        self.event.is_some()
    }

    pub fn data(&self) -> SharedTextureInfo {
        self.data.lock().unwrap().clone()
    }
}
