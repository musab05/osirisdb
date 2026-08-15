use crate::storage::{StorageError, util::checksum::crc32c};

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
pub struct LogRecord {
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

impl LogRecord {
    /// Computes the total byte size of this record when serialized to disk.
    pub fn size(&self) -> usize {
        8 + 8 + 8 + 1 + // lsn, prev_lsn, txn_id, record_type
        4 + 4 + 2 + 2 + // file_id, page_id, offset, length
        4 + self.before_image.len() + // before_image length + data
        4 + self.after_image.len() + // after_image length + data
        4 // crc32c
    }

    pub fn serialize(&self) -> Vec<u8> {
        // Create a buffer with enough capacity based on the size method
        let mut buffer = Vec::with_capacity(self.size());

        // 1. Write the fixed sized 64 bit integers (8 bytes each)
        buffer.extend_from_slice(&self.lsn.to_le_bytes());
        buffer.extend_from_slice(&self.prev_lsn.to_le_bytes());
        buffer.extend_from_slice(&self.txt_id.to_le_bytes());

        // 2. Write the Recordtype enum as a single type
        buffer.push(self.record_type as u8);

        // 3. Write the rest of the fixed sized integers
        buffer.extend_from_slice(&self.file_id.to_le_bytes());
        buffer.extend_from_slice(&self.page_id.to_le_bytes());
        buffer.extend_from_slice(&self.offset.to_le_bytes());
        buffer.extend_from_slice(&self.length.to_le_bytes());

        // 4. write the rest of the length data
        let before_len = self.before_image.len() as u32;
        buffer.extend_from_slice(&before_len.to_le_bytes());
        buffer.extend_from_slice(&self.before_image);

        let after_len = self.after_image.len() as u32;
        buffer.extend_from_slice(&after_len.to_le_bytes());
        buffer.extend_from_slice(&self.after_image);

        // 5. Calculate crc32c over everything serialized
        let checksum = crc32c(&buffer);

        buffer.extend_from_slice(&checksum.to_le_bytes());

        buffer
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self, StorageError> {
        // We need at least the fixed fields + 4 byte CRC to even try parsing
        // (8*3) + 1 + (4*2) + (2*2) + (4*2 for vec lengths) + 4 (crc) = 45 bytes minimum
        if bytes.len() < 45 {
            return Err(StorageError::LogRecordTooSmall);
        }

        // 1. Check the CRC32C Checksum
        // The last 4 bytes are the checksum. Everything before is the data.
        let data_len = bytes.len() - 4;
        let data_bytes = &bytes[0..data_len];
        let crc_bytes = &bytes[data_len..];

        let expected_checksum = u32::from_le_bytes(crc_bytes.try_into().unwrap());

        let checksum = crc32c(data_bytes);

        if checksum != expected_checksum {
            return Err(StorageError::ChecksumMismatch);
        }

        // 2. Start reading fields using a cursor/offset tracking where we are
        let mut cursor = 0;

        // Helper closer to read fixed size chunks
        let mut read_u64 = || {
            let val = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
            cursor += 8;
            val
        };

        let lsn = read_u64();
        let prev_lsn = read_u64();
        let txt_id = read_u64();

        // Read RecordType
        let record_type =
            RecordType::try_from(bytes[cursor]).map_err(|_| StorageError::InvalidRecordType)?;
        cursor += 1;

        let file_id = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
        cursor += 4;
        let page_id = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
        cursor += 4;
        let offset = u16::from_le_bytes(bytes[cursor..cursor + 2].try_into().unwrap());
        cursor += 2;
        let length = u16::from_le_bytes(bytes[cursor..cursor + 2].try_into().unwrap());
        cursor += 2;

        // 3. Read variable length vectors
        let before_len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;
        let before_image = bytes[cursor..cursor + before_len].to_vec();
        cursor += before_len;
        let after_len = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;
        let after_image = bytes[cursor..cursor + after_len].to_vec();
        // cursor += after_len; // cursor is at the end now (except for CRC which we already stripped)

        Ok(LogRecord {
            lsn,
            prev_lsn,
            txt_id,
            record_type,
            file_id,
            page_id,
            offset,
            length,
            before_image,
            after_image,
        })
    }
}
