use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    unreachable, vec,
};

use crate::storage::{
    CheckpointData, FileRegistry, HeapFile, LogManager, StorageError,
    log::log_record::{LogRecord, RecordType},
};

pub struct RecoveryEngine {
    log_path: PathBuf,
    meta_path: PathBuf,
    file_registry: Arc<FileRegistry>,
    log_manager: Arc<LogManager>,
}

impl RecoveryEngine {
    pub fn new(
        log_path: impl AsRef<Path>,
        meta_path: impl AsRef<Path>,
        file_registry: Arc<FileRegistry>,
        log_manager: Arc<LogManager>,
    ) -> Self {
        Self {
            log_path: log_path.as_ref().to_path_buf(),
            meta_path: meta_path.as_ref().to_path_buf(),
            file_registry,
            log_manager,
        }
    }

    /// Helper that sequentially reads and deserializes all [`LogRecord`]s from the WAL file.
    pub fn read_log_records(log_path: &Path) -> Result<Vec<LogRecord>, StorageError> {
        if !log_path.exists() {
            return Ok(Vec::new());
        }

        let mut file = File::open(log_path).map_err(|e| StorageError::io(log_path, e))?;

        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .map_err(|e| StorageError::io(log_path, e))?;

        let mut records = Vec::new();
        let mut cursor = 0;

        while cursor + 45 <= bytes.len() {
            let before_len =
                u32::from_le_bytes(bytes[cursor + 37..cursor + 41].try_into().unwrap()) as usize;
            if cursor + 45 + before_len > bytes.len() {
                break;
            }

            let after_len = u32::from_le_bytes(
                bytes[cursor + 41 + before_len..cursor + 45 + before_len]
                    .try_into()
                    .unwrap(),
            ) as usize;
            let record_size = 49 + before_len + after_len;

            if cursor + record_size > bytes.len() {
                break;
            }

            let rec_slice = &bytes[cursor..cursor + record_size];
            let record = LogRecord::deserialize(rec_slice)?;
            records.push(record);
            cursor += record_size;
        }

        Ok(records)
    }

    /// Analyze - Returns: (ATT: txn_id -> last_lsn, DPT: (file_id, page_id) -> rec_lsn, redo_lsn)
    pub fn analyze(
        &self,
        records: &[LogRecord],
        start_lsn: u64,
    ) -> (HashMap<u64, u64>, HashMap<(u32, u32), u64>, u64) {
        let mut att: HashMap<u64, u64> = HashMap::new();
        let mut dpt: HashMap<(u32, u32), u64> = HashMap::new();

        for record in records {
            // only inscpecting records from start_lsn onwards
            if record.lsn < start_lsn {
                continue;
            }

            match record.record_type {
                RecordType::Begin => {
                    // Transaction statrted Add to ATT: txn_id -> record.lsn
                    att.insert(record.txt_id, record.lsn);
                }
                RecordType::Commit | RecordType::Abort => {
                    // Transaction finished Remove from ATT
                    att.remove(&record.txt_id);
                }
                RecordType::Insert | RecordType::Update | RecordType::Delete => {
                    // Update ATT: transaction's latest LSN is this record
                    att.insert(record.txt_id, record.lsn);

                    // Update DPT: If this page isn't in DPT yet, set rec_lsn to this record's LSN
                    let page_key = (record.file_id, record.page_id);
                    dpt.entry(page_key).or_insert(record.lsn);
                }
                RecordType::Compensation => {
                    // CLR record: update txn's last_lsn
                    att.insert(record.txt_id, record.lsn);
                }
                RecordType::CheckpointEnd => {
                    // Load snapshot of active transactions and dirty pages from after_image!
                    if let Ok(ckpt_data) = CheckpointData::deserialize(&record.after_image) {
                        for (txn_id, last_lsn) in ckpt_data.active_txns {
                            att.entry(txn_id).or_insert(last_lsn);
                        }
                        for ((file_id, page_id), rec_lsn) in ckpt_data.dirty_pages {
                            dpt.entry((file_id, page_id)).or_insert(rec_lsn);
                        }
                    }
                }
                RecordType::CheckpointBegin => {}
            }
        }

        // redo_lsn is the smallest rec_lsn in the DPT (or start_lsn if DPT is empty)
        let redo_lsn = dpt.values().copied().min().unwrap_or(start_lsn);

        (att, dpt, redo_lsn)
    }

    /// Redo — Re-applies all missing changes from redo_lsn forward.
    pub fn redo(
        &self,
        records: &[LogRecord],
        redo_lsn: u64,
        dpt: &HashMap<(u32, u32), u64>,
    ) -> Result<(), StorageError> {
        for record in records {
            // only process records at or after redo_lsn
            if record.lsn < redo_lsn {
                continue;
            }

            // Check if this record type modifies physical pages
            match record.record_type {
                RecordType::Insert
                | RecordType::Update
                | RecordType::Delete
                | RecordType::Compensation => {}
                _ => continue, // Skip Begin, Commit, Abort, Checkpoints
            }

            // DPT Check: If page is not in DPT or record is before rec_lsn, skip
            let page_key = (record.file_id, record.page_id);
            if let Some(&rec_lsn) = dpt.get(&page_key) {
                if record.lsn < rec_lsn {
                    continue;
                }
            } else {
                continue;
            }

            //Look up physical file path using FileRegistry
            let Some(file_path) = self.file_registry.get_path(record.file_id) else {
                continue;
            };
            if !file_path.exists() {
                continue;
            }

            let mut heap_file = HeapFile::open(&file_path)?;

            // If the page doesn't exist yet on disk (e.g. newly allocated before crash), allocate pages
            while heap_file.num_pages <= record.page_id {
                heap_file.allocate_page()?;
            }

            let mut page = heap_file.read_page(record.page_id)?;

            // THE GOLDEN CHECK: If page on disk already has this change, skip!
            if page.page_lsn() >= record.lsn {
                continue;
            }

            // Re-apply the operation to the page
            match record.record_type {
                RecordType::Insert | RecordType::Compensation => {
                    page.insert_tuple(&record.after_image);
                }
                RecordType::Update => {
                    page.delete_tuple(record.offset);
                    page.insert_tuple(&record.after_image);
                }
                RecordType::Delete => {
                    page.delete_tuple(record.offset);
                }

                _ => unreachable!(),
            }

            //  Update page_lsn to this record's LSN, recompute checksum, and write to disk
            page.set_page_lsn(record.lsn);
            page.compute_checksum();
            heap_file.write_page(record.page_id, &page)?;
        }

        Ok(())
    }

    /// Undo — Rolls back all active/uncommitted transactions backward.
    pub fn undo(
        &self,
        records: &[LogRecord],
        att: HashMap<u64, u64>, // loser transactions: txn_id -> last_lsn
    ) -> Result<(), StorageError> {
        if att.is_empty() {
            return Ok(());
        }

        // Build an index for O(1) lookup: lsn -> &LogRecord
        let lsn_map: HashMap<u64, &LogRecord> = records.iter().map(|r| (r.lsn, r)).collect();

        // Active undo list containing the next LSN to undo for each loser transaction
        let mut to_undo: HashMap<u64, u64> = att;

        // Process until all loser transactions are completely rolled back
        while !to_undo.is_empty() {
            // Pick the transaction with the largest LSN to process next
            let (&max_txn_id, &max_lsn) = to_undo.iter().max_by_key(|&(_, &lsn)| lsn).unwrap();

            let Some(&record) = lsn_map.get(&max_lsn) else {
                to_undo.remove(&max_txn_id);
                continue;
            };

            // If this record modified a page, apply the reverse (undo) operation
            match record.record_type {
                RecordType::Insert => {
                    // Undo Insert: Delete the inserted tuple from the page
                    if let Some(file_path) = self.file_registry.get_path(record.file_id) {
                        if file_path.exists() {
                            let mut heap_file = HeapFile::open(&file_path)?;
                            if record.page_id < heap_file.num_pages {
                                let mut page = heap_file.read_page(record.page_id)?;
                                page.delete_tuple(record.offset);
                                page.compute_checksum();
                                heap_file.write_page(record.page_id, &page)?;
                            }
                        }
                    }
                    // Write a Compensation Log Record (CLR)
                    let mut clr = LogRecord {
                        lsn: 0,
                        prev_lsn: record.prev_lsn,
                        txt_id: record.txt_id,
                        record_type: RecordType::Compensation,
                        file_id: record.file_id,
                        page_id: record.page_id,
                        offset: record.offset,
                        length: 0,
                        before_image: vec![],
                        after_image: vec![],
                    };
                    self.log_manager.append_record(&mut clr)?;
                }
                RecordType::Delete => {
                    // Undo Delete: Re-insert the old tuple (before_image)
                    if let Some(file_path) = self.file_registry.get_path(record.file_id) {
                        if file_path.exists() {
                            let mut heap_file = HeapFile::open(&file_path)?;
                            if record.page_id < heap_file.num_pages {
                                let mut page = heap_file.read_page(record.page_id)?;
                                page.insert_tuple(&record.before_image);
                                page.compute_checksum();
                                heap_file.write_page(record.page_id, &page)?;
                            }
                        }
                    }
                    // Write CLR with the restored bytes
                    let mut clr = LogRecord {
                        lsn: 0,
                        prev_lsn: record.prev_lsn,
                        txt_id: record.txt_id,
                        record_type: RecordType::Compensation,
                        file_id: record.file_id,
                        page_id: record.page_id,
                        offset: record.offset,
                        length: record.before_image.len() as u16,
                        before_image: vec![],
                        after_image: record.before_image.clone(),
                    };
                    self.log_manager.append_record(&mut clr)?;
                }
                RecordType::Update => {
                    // Undo Update: Revert the slot back to before_image
                    if let Some(file_path) = self.file_registry.get_path(record.file_id) {
                        if file_path.exists() {
                            let mut heap_file = HeapFile::open(&file_path)?;
                            if record.page_id < heap_file.num_pages {
                                let mut page = heap_file.read_page(record.page_id)?;
                                page.delete_tuple(record.offset);
                                page.insert_tuple(&record.before_image);
                                page.compute_checksum();
                                heap_file.write_page(record.page_id, &page)?;
                            }
                        }
                    }
                    // Write CLR
                    let mut clr = LogRecord {
                        lsn: 0,
                        prev_lsn: record.prev_lsn,
                        txt_id: record.txt_id,
                        record_type: RecordType::Compensation,
                        file_id: record.file_id,
                        page_id: record.page_id,
                        offset: record.offset,
                        length: record.before_image.len() as u16,
                        before_image: vec![],
                        after_image: record.before_image.clone(),
                    };
                    self.log_manager.append_record(&mut clr)?;
                }
                RecordType::Compensation => {
                    // CLR records are NEVER undone — just follow prev_lsn!
                }
                _ => {}
            }

            // Follow the backward prev_lsn chain:
            if record.prev_lsn == 0 {
                // We reached the transaction's BEGIN record - write ABORT record!
                let mut abort_record = LogRecord {
                    lsn: 0,
                    prev_lsn: 0,
                    txt_id: max_txn_id,
                    record_type: RecordType::Abort,
                    file_id: 0,
                    page_id: 0,
                    offset: 0,
                    length: 0,
                    before_image: vec![],
                    after_image: vec![],
                };

                self.log_manager.append_record(&mut abort_record)?;

                // Transaction is completely rolled back Remove from loser list
                to_undo.remove(&max_txn_id);
            } else {
                // Move backward to predecessor record
                to_undo.insert(max_txn_id, record.prev_lsn);
            }
        }

        // Flush all CLR and Abort records to disk
        self.log_manager.flush()?;
        Ok(())
    }

    /// Executes the complete 3-phase ARIES crash recovery algorithm.
    pub fn recover(&self) -> Result<(), StorageError> {
        // 1. Read master record (checkpoint.meta) to find checkpoint start LSN
        let ckpt_begin_lsn = if self.meta_path.exists() {
            let meta_bytes =
                std::fs::read(&self.meta_path).map_err(|e| StorageError::io(&self.meta_path, e))?;
            if meta_bytes.len() >= 8 {
                u64::from_le_bytes(meta_bytes[0..8].try_into().unwrap())
            } else {
                1
            }
        } else {
            1 // No checkpoint exists, start analysis from LSN 1
        };

        // 2. Read all sequential WAL records from disk
        let records = Self::read_log_records(&self.log_path)?;
        if records.is_empty() {
            return Ok(());
        }

        // 3. Phase 1: Analysis
        let (att, dpt, redo_lsn) = self.analyze(&records, ckpt_begin_lsn);

        // 4. Phase 2: Redo (Repeating History)
        self.redo(&records, redo_lsn, &dpt)?;

        // 5. Phase 3: Undo (Rolling Back Loser Transactions)
        self.undo(&records, att)?;

        Ok(())
    }
}
