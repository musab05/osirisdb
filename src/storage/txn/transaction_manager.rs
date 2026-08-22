use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    todo,
};

use crate::storage::{
    StorageError,
    log::{
        log_manager::LogManager,
        log_record::{LogRecord, RecordType},
    },
    txn::transaction::Transaction,
};

pub struct TransactionManager {
    /// Atomic generator for unique transaction IDs.
    next_txn_id: AtomicU64,

    /// Active transaction table (ATT) tracking in-flight transactions.
    active_txns: Mutex<HashMap<u64, Transaction>>,

    /// Shared global WAL log manager.
    log_manager: Arc<LogManager>,
}

impl TransactionManager {
    pub fn new(log_manager: Arc<LogManager>) -> Self {
        Self {
            next_txn_id: AtomicU64::new(1), // start at 1 (0 = "no txn")
            active_txns: Mutex::new(HashMap::new()),
            log_manager,
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

        // Create the Transaction object with last_lsn pointing to BEGIN\
        let txn = Transaction::new(txn_id);

        // Set last_lsn
        let mut txn = txn;
        txn.last_lsn = lsn.0;

        // Insert into Active Transaction Table
        self.active_txns.lock().unwrap().insert(txn_id, txn.clone());

        Ok(txn)
    }
}
