#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordType {
    Insert = 0,
    Delete = 1,
    Update = 2,
    Begin = 3,
    Commit = 4,
    Abort = 5,
    CheckpointBegin = 6,
    CheckpointEnd = 7,
    Compensation = 8, // CLR for undo
}

impl TryFrom<u8> for RecordType {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(RecordType::Insert),
            1 => Ok(RecordType::Delete),
            2 => Ok(RecordType::Update),
            3 => Ok(RecordType::Begin),
            4 => Ok(RecordType::Commit),
            5 => Ok(RecordType::Abort),
            6 => Ok(RecordType::CheckpointBegin),
            7 => Ok(RecordType::CheckpointEnd),
            8 => Ok(RecordType::Compensation),
            _ => Err(()),
        }
    }
}

/// Represents a single physiological WAL record.
#[derive(Debug, Clone)]
pub struct logRecord {
    pub lsn: u64,
    pub prev_lsn: u64,
    pub txt_id: u64,
    pub record_type: RecordType,

    // These are only strictly necessary for physiological operations (Insert, Update, Delete)
    pub file_id: u32,
    pub page_id: u32,
    pub offset: u16,
    pub length: u16,

    pub before_image: Vec<u8>,
    pub after_image: Vec<u8>,
    // Note: crc32c will be computed right before writing to disk,
    // it doesn't strictly need to be a field in the memory representation.
}

impl logRecord {
    /// Computes the total byte size of this record when serialized to disk.
    pub fn size(&self) -> usize {
        8 + 8 + 8 + 1 + // lsn, prev_lsn, txn_id, record_type
        4 + 4 + 2 + 2 + // file_id, page_id, offset, length
        4 + self.before_image.len() + // before_image length + data
        4 + self.after_image.len() + // after_image length + data
        4 // crc32c
    }
}
