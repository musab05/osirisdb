use std::vec;

use crate::storage::StorageError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointData {
    pub active_txns: Vec<(u64, u64)>,        // (txn_id, last_lsn)
    pub dirty_pages: Vec<((u32, u32), u64)>, // ((file_id, page_id), rec_lsn)
}

impl CheckpointData {
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::new();

        // Serialize active_txns
        bytes.extend_from_slice(&(self.active_txns.len() as u32).to_le_bytes());
        for (txn_id, last_lsn) in &self.active_txns {
            bytes.extend_from_slice(&txn_id.to_le_bytes());
            bytes.extend_from_slice(&last_lsn.to_le_bytes());
        }

        // Serialize dirty pages
        bytes.extend_from_slice(&(self.dirty_pages.len() as u32).to_le_bytes());
        for ((file_id, page_id), rec_lsn) in &self.dirty_pages {
            bytes.extend_from_slice(&file_id.to_le_bytes());
            bytes.extend_from_slice(&page_id.to_le_bytes());
            bytes.extend_from_slice(&rec_lsn.to_le_bytes());
        }

        bytes
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self, StorageError> {
        if bytes.len() < 8 {
            return Ok(Self {
                active_txns: vec![],
                dirty_pages: vec![],
            });
        }

        let mut cursor = 0;

        // Read active txns count
        let txns_count = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;

        let mut active_txns = Vec::with_capacity(txns_count);
        for _ in 0..txns_count {
            if cursor + 16 > bytes.len() {
                break;
            }
            let txn_id = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
            cursor += 8;
            let last_lsn = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
            cursor += 8;
            active_txns.push((txn_id, last_lsn));
        }

        // Read dirty pages count
        if cursor + 4 <= bytes.len() {
            let pages_count =
                u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap()) as usize;
            cursor += 4;

            let mut dirty_pages = Vec::with_capacity(pages_count);
            for _ in 0..pages_count {
                if cursor + 16 > bytes.len() {
                    break;
                }
                let file_id = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
                cursor += 4;
                let page_id = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
                cursor += 4;
                let rec_lsn = u64::from_le_bytes(bytes[cursor..cursor + 8].try_into().unwrap());
                cursor += 8;
                dirty_pages.push(((file_id, page_id), rec_lsn));
            }
            Ok(Self {
                active_txns,
                dirty_pages,
            })
        } else {
            Ok(Self {
                active_txns,
                dirty_pages: vec![],
            })
        }
    }
}
