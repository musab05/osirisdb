use std::sync::RwLock;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClogStatus {
    InProgress = 0b00,
    Committed = 0b01,
    Aborted = 0b10,
}

impl From<u8> for ClogStatus {
    fn from(val: u8) -> Self {
        match val & 0b11 {
            0b01 => ClogStatus::Committed,
            0b10 => ClogStatus::Aborted,
            _ => ClogStatus::InProgress,
        }
    }
}

pub struct Clog {
    /// In memory bytes array or page buffer mapping txn_id to status bits.
    /// Can starts as a concurrent memory array or file backed buffer.
    status_bytes: RwLock<Vec<u8>>,
}

impl Clog {
    pub fn new() -> Self {
        Self {
            status_bytes: RwLock::new(Vec::new()),
        }
    }

    /// Sets teh status of transaction `txn_id`
    pub fn set_status(&self, txn_id: u64, status: ClogStatus) {
        if txn_id == 0 {
            return;
        }

        let byte_idx = (txn_id / 4) as usize;
        let bit_offset = ((txn_id % 4) * 2) as u8;

        let mut lock = self.status_bytes.write().unwrap();
        if byte_idx >= lock.len() {
            lock.resize(byte_idx + 1024, 0); // grow with headroom
        }

        // Clear 2 bits and set new status
        lock[byte_idx] &= !(0b11 << bit_offset);
        lock[byte_idx] |= (status as u8) << bit_offset;
    }

    /// Read the status of transaction `txn_id`
    pub fn get_status(&self, txn_id: u64) -> ClogStatus {
        if txn_id == 0 {
            return ClogStatus::Committed; // txn 0 = bootstrap/frozen
        }

        let byte_idx = (txn_id / 4) as usize;
        let bit_offset = ((txn_id % 4) * 2) as u8;

        let lock = self.status_bytes.read().unwrap();
        if byte_idx >= lock.len() {
            return ClogStatus::InProgress;
        }

        let raw = (lock[byte_idx] >> bit_offset) & 0b11;
        ClogStatus::from(raw)
    }
}
