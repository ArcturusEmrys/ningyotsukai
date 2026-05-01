use std::ffi::CStr;

pub const MAX_SENDER_LEN: usize = 255;

/// A sender name as represented in Spout2.
///
/// Intended to be null-terminated.
/// The first byte being null indicates no value.
#[derive(Debug, Copy, Clone, bytemuck::Zeroable, bytemuck::Pod)]
#[repr(transparent)]
pub struct SenderName(pub(crate) [u8; MAX_SENDER_LEN + 1]);

impl SenderName {
    pub fn as_cstr(&self) -> Option<&CStr> {
        if self.0[0] == 0 {
            return None;
        }

        CStr::from_bytes_until_nul(&self.0).ok()
    }

    pub fn try_from_cstr(data: Option<&CStr>) -> Option<Self> {
        let mut out = [0; MAX_SENDER_LEN + 1];

        if let Some(data) = data {
            if data.count_bytes() > MAX_SENDER_LEN {
                return None;
            }

            let bytes = data.to_bytes_with_nul();

            out[0..bytes.len()].copy_from_slice(bytes);
        }

        Some(Self(out))
    }

    pub fn invalid() -> Self {
        Self([0; MAX_SENDER_LEN + 1])
    }
}

impl PartialEq for SenderName {
    fn eq(&self, other: &Self) -> bool {
        for (self_byte, other_byte) in self.0.iter().zip(other.0.iter()) {
            if *self_byte != *other_byte {
                return false;
            }

            if *self_byte == 0 {
                return true;
            }
        }

        true
    }
}
