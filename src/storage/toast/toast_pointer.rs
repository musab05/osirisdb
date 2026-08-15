use crate::storage::StorageError;

pub const TOAST_THRESHOLD: usize = 2042; // (8192 - 24) / 4
pub const TOAST_TAG_INLINE: u8 = 0x00;
pub const TOAST_TAG_POINTER: u8 = 0x01;
pub const TOAST_POINTER_SIZE: usize = 10;

/// A 10-byte inline reference to an out-of-line TOAST payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToastPointer {
    pub total_length: u32,
    pub first_page_id: u32,
}

impl ToastPointer {
    pub fn to_bytes(&self) -> [u8; TOAST_POINTER_SIZE] {
        let mut buf = [0u8; TOAST_POINTER_SIZE];
        buf[0] = TOAST_TAG_POINTER;
        buf[1] = 0x00; // reserved
        buf[2..6].copy_from_slice(&self.total_length.to_le_bytes());
        buf[6..10].copy_from_slice(&self.first_page_id.to_le_bytes());
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, StorageError> {
        if bytes.len() < TOAST_POINTER_SIZE || bytes[0] != TOAST_TAG_POINTER {
            return Err(StorageError::TupleError("invalid TOAST pointer tag".into()));
        }

        let total_length = u32::from_le_bytes(bytes[2..6].try_into().unwrap());
        let first_page_id = u32::from_le_bytes(bytes[6..10].try_into().unwrap());
        Ok(Self {
            total_length,
            first_page_id,
        })
    }
}
