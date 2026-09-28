use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::storage::{
    BufferPool, FileRegistry, HeapFile, RecoveryEngine, StorageError,
    log::{
        log_manager::LogManager,
        log_record::{LogRecord, RecordType},
    },
    txn::{
        clog::{Clog, ClogStatus},
        snapshot::Snapshot,
        transaction::{Transaction, TxnStatus},
    },
};

pub struct TransactionManager {
    /// Atomic generator for unique transaction IDs.
    next_txn_id: AtomicU64,

    /// Active transaction table (ATT) tracking in-flight transactions.
    active_txns: Mutex<HashMap<u64, Transaction>>,

    /// Shared global WAL log manager.
    log_manager: Arc<LogManager>,

    /// Clog
    clog: Arc<Clog>,

    file_registry: Arc<FileRegistry>,

    buffer_pool: Arc<Mutex<BufferPool>>,
}

impl TransactionManager {
    pub fn new(
        log_manager: Arc<LogManager>,
        file_registry: Arc<FileRegistry>,
        buffer_pool: Arc<Mutex<BufferPool>>,
    ) -> Self {
        Self {
            next_txn_id: AtomicU64::new(1), // start at 1 (0 = "no txn")
            active_txns: Mutex::new(HashMap::new()),
            log_manager,
            clog: Arc::new(Clog::new()),
            file_registry,
            buffer_pool,
        }
    }

    pub fn begin(&self) -> Result<Transaction, StorageError> {
        // Atomically generate a unique transaction ID
        let txn_id = self.next_txn_id.fetch_add(1, Ordering::SeqCst);

        // Create a BEGIN log record (no page data, just lifecycle)
        let mut record = LogRecord {
            lsn: 0,      // LogManager is going to assign the real lsn
            prev_lsn: 0, // First record in this transaction chain -> no predecessor
            txt_id: txn_id,
            record_type: RecordType::Begin,
            file_id: 0, // N/A for lifecycle records
            page_id: 0,
            offset: 0,
            length: 0,
            before_image: Vec::new(),
            after_image: Vec::new(),
        };

        // Append to WAL, get the assigned LSN
        let lsn = self.log_manager.append_record(&mut record)?;

        // Create the Transaction object with last_lsn pointing to BEGIN
        let mut txn = Transaction::new(txn_id);
        txn.last_lsn = lsn.0;

        // Insert into Active Transaction Table
        self.active_txns.lock().unwrap().insert(txn_id, txn.clone());

        // mark as InProgress in CLOG
        self.clog.set_status(txn_id, ClogStatus::InProgress);

        Ok(txn)
    }

    pub fn commit(&self, txn: &mut Transaction) -> Result<(), StorageError> {
        // Creating COMMIT log record, chaining prev_lsn to txn's last record
        let mut record = LogRecord {
            lsn: 0,
            prev_lsn: txn.last_lsn, // Backward chain link
            txt_id: txn.txn_id,
            record_type: RecordType::Commit,
            file_id: 0,
            page_id: 0,
            offset: 0,
            length: 0,
            before_image: Vec::new(),
            after_image: Vec::new(),
        };

        // Append to WAL
        let lsn = self.log_manager.append_record(&mut record)?;
        txn.last_lsn = lsn.0;

        // DURABILITY: wait until the commit record is fsynced to disk
        //      This is where Group Commit kicks in - multiple committing txns
        //      will all block here and be woken by one fync from the flusher thread
        self.log_manager.wait_for_flush(lsn.0)?;

        // Update transaction status
        txn.status = TxnStatus::Committed;

        // Record committed status in CLOG
        self.clog.set_status(txn.txn_id, ClogStatus::Committed);

        // Remove from Active Transaction Table
        self.active_txns.lock().unwrap().remove(&txn.txn_id);

        Ok(())
    }

    pub fn abort(&self, txn: &mut Transaction) -> Result<(), StorageError> {
        // Read WAL records from disk to build the backward chain
        let log_path = self.log_manager.log_path();
        let records = RecoveryEngine::read_log_records(&log_path)?;
        let lsn_map: HashMap<u64, &LogRecord> = records.iter().map(|r| (r.lsn, r)).collect();

        // Start from txn.last_lsn and walk backward
        let mut next_lsn = txn.last_lsn;
        while next_lsn != 0 {
            let Some(&record) = lsn_map.get(&next_lsn) else {
                break; // reached beginning of chain or missing record
            };

            // Advance backward pointer immediately so any `continue` in branches advances safely
            next_lsn = record.prev_lsn;

            match record.record_type {
                RecordType::Insert => {
                    // Undo Insert: delete the inserted tuple from the buffer pool page
                    let mut bp = self.buffer_pool.lock().unwrap();

                    let frame_id = match bp.pin_page(record.file_id, record.page_id) {
                        Ok(frame) => frame,
                        Err(StorageError::UnknownFile(_)) => {
                            if let Some(file_path) = self.file_registry.get_path(record.file_id) {
                                if file_path.exists() {
                                    let hf = HeapFile::open(&file_path)?;
                                    bp.register_file(record.file_id, hf);
                                    bp.pin_page(record.file_id, record.page_id)?
                                } else {
                                    continue;
                                }
                            } else {
                                continue;
                            }
                        }
                        Err(e) => return Err(e),
                    };

                    bp.get_page_mut(frame_id).delete_tuple(record.offset);

                    let mut clr = LogRecord {
                        lsn: 0,
                        prev_lsn: record.prev_lsn,
                        txt_id: txn.txn_id,
                        record_type: RecordType::Compensation,
                        file_id: record.file_id,
                        page_id: record.page_id,
                        offset: record.offset,
                        length: 0,
                        before_image: Vec::new(),
                        after_image: Vec::new(),
                    };
                    let clr_lsn = self.log_manager.append_record(&mut clr)?;
                    txn.last_lsn = clr_lsn.0;

                    bp.get_page_mut(frame_id).set_page_lsn(clr_lsn.0);
                    bp.unpin_page(frame_id, true);
                }
                RecordType::Delete => {
                    // Undo Delete: re-insert before_image into the buffer pool page
                    let mut bp = self.buffer_pool.lock().unwrap();

                    let frame_id = match bp.pin_page(record.file_id, record.page_id) {
                        Ok(frame) => frame,
                        Err(StorageError::UnknownFile(_)) => {
                            if let Some(file_path) = self.file_registry.get_path(record.file_id) {
                                if file_path.exists() {
                                    let hf = HeapFile::open(&file_path)?;
                                    bp.register_file(record.file_id, hf);
                                    bp.pin_page(record.file_id, record.page_id)?
                                } else {
                                    continue;
                                }
                            } else {
                                continue;
                            }
                        }
                        Err(e) => return Err(e),
                    };

                    bp.get_page_mut(frame_id).insert_tuple(&record.before_image);

                    let mut clr = LogRecord {
                        lsn: 0,
                        prev_lsn: record.prev_lsn,
                        txt_id: txn.txn_id,
                        record_type: RecordType::Compensation,
                        file_id: record.file_id,
                        page_id: record.page_id,
                        offset: record.offset,
                        length: record.before_image.len() as u16,
                        before_image: Vec::new(),
                        after_image: record.before_image.clone(),
                    };
                    let clr_lsn = self.log_manager.append_record(&mut clr)?;
                    txn.last_lsn = clr_lsn.0;

                    bp.get_page_mut(frame_id).set_page_lsn(clr_lsn.0);
                    bp.unpin_page(frame_id, true);
                }
                RecordType::Update => {
                    // Undo Update: delete updated tuple, re-insert before_image
                    let mut bp = self.buffer_pool.lock().unwrap();

                    let frame_id = match bp.pin_page(record.file_id, record.page_id) {
                        Ok(frame) => frame,
                        Err(StorageError::UnknownFile(_)) => {
                            if let Some(file_path) = self.file_registry.get_path(record.file_id) {
                                if file_path.exists() {
                                    let hf = HeapFile::open(&file_path)?;
                                    bp.register_file(record.file_id, hf);
                                    bp.pin_page(record.file_id, record.page_id)?
                                } else {
                                    continue;
                                }
                            } else {
                                continue;
                            }
                        }
                        Err(e) => return Err(e),
                    };

                    bp.get_page_mut(frame_id).delete_tuple(record.offset);
                    bp.get_page_mut(frame_id).insert_tuple(&record.before_image);

                    let mut clr = LogRecord {
                        lsn: 0,
                        prev_lsn: record.prev_lsn,
                        txt_id: txn.txn_id,
                        record_type: RecordType::Compensation,
                        file_id: record.file_id,
                        page_id: record.page_id,
                        offset: record.offset,
                        length: record.before_image.len() as u16,
                        before_image: Vec::new(),
                        after_image: record.before_image.clone(),
                    };
                    let clr_lsn = self.log_manager.append_record(&mut clr)?;
                    txn.last_lsn = clr_lsn.0;

                    bp.get_page_mut(frame_id).set_page_lsn(clr_lsn.0);
                    bp.unpin_page(frame_id, true);
                }
                RecordType::Compensation => {
                    // CLRs are never undone — follow prev_lsn to bypass already undone operations
                }
                _ => {} // Begin, Commit, Abort — skip
            }
        }

        // Create ABORT log record (prev_lsn points to last CLR or operation)
        let mut record = LogRecord {
            lsn: 0,
            prev_lsn: txn.last_lsn,
            txt_id: txn.txn_id,
            record_type: RecordType::Abort,
            file_id: 0,
            page_id: 0,
            offset: 0,
            length: 0,
            before_image: Vec::new(),
            after_image: Vec::new(),
        };

        // Append to WAL
        let lsn = self.log_manager.append_record(&mut record)?;
        txn.last_lsn = lsn.0;

        // Update transaction status
        txn.status = TxnStatus::Aborted;

        // Record aborted status in CLOG
        self.clog.set_status(txn.txn_id, ClogStatus::Aborted);

        // Remove from Active Transaction Table
        self.active_txns.lock().unwrap().remove(&txn.txn_id);

        Ok(())
    }

    /// Returns a copy of the active transaction state for `txn_id`, if it is currently in flight.
    pub fn get_active_txn(&self, txn_id: u64) -> Option<Transaction> {
        self.active_txns.lock().unwrap().get(&txn_id).cloned()
    }

    /// Returns the number of currently active in-flight transactions.
    pub fn active_txn_count(&self) -> usize {
        self.active_txns.lock().unwrap().len()
    }

    /// Returns a reference to the shared [`LogManager`].
    pub fn log_manager(&self) -> &Arc<LogManager> {
        &self.log_manager
    }

    /// Returns a snapshot of all active transactions as (txn_id, last_lsn) pairs.
    pub fn get_active_transactions(&self) -> Vec<(u64, u64)> {
        self.active_txns
            .lock()
            .unwrap()
            .values()
            .map(|txn| (txn.txn_id, txn.last_lsn))
            .collect()
    }

    /// Capturing snapshot inside TransactionManager
    pub fn take_snapshot(&self) -> Snapshot {
        let active = self.active_txns.lock().unwrap();
        let next_id = self.next_txn_id.load(Ordering::SeqCst);

        let mut xip_list: Vec<u64> = active.keys().cloned().collect();
        xip_list.sort_unstable();

        let xmin = xip_list.first().cloned().unwrap_or(next_id);
        let xmax = next_id;

        Snapshot {
            xmin,
            xmax,
            xip_list,
        }
    }

    /// Returns a reference to the shared [`Clog`]
    pub fn clog(&self) -> &Arc<Clog> {
        &self.clog
    }
}
