/// References a specific row location inside a TableHeap file.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct RecordId {
    pub page_id: u32,
    pub slot_id: u16,
}

impl RecordId {
    /// Serializes the RecordId into a fixed 6-byte array for disk storage.
    pub fn to_bytes(&self) -> [u8; 6] {
        let mut bytes = [0u8; 6];
        bytes[0..4].copy_from_slice(&self.page_id.to_le_bytes());
        bytes[4..6].copy_from_slice(&self.slot_id.to_le_bytes());
        bytes
    }

    /// Deserializes the RecordId from a fixed 6-byte array.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let page_id = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let slot_id = u16::from_le_bytes(bytes[4..6].try_into().unwrap());
        RecordId { page_id, slot_id }
    }
}
