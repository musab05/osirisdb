use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use crate::storage::{
    StorageError,
    log::{
        log_manager::LogManager,
        log_record::{LogRecord, RecordType},
    },
    txn::transaction::{Transaction, TxnStatus},
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

        // Create the Transaction object with last_lsn pointing to BEGIN
        let mut txn = Transaction::new(txn_id);
        txn.last_lsn = lsn.0;

        // Insert into Active Transaction Table
        self.active_txns.lock().unwrap().insert(txn_id, txn.clone());

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

        // Remove from Active Transaction Table
        self.active_txns.lock().unwrap().remove(&txn.txn_id);

        Ok(())
    }

    pub fn abort(&self, txn: &mut Transaction) -> Result<(), StorageError> {
        // Create ABORT log record
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

        // Remove from Active Transaction Table
        self.active_txns.lock().unwrap().remove(&txn.txn_id);

        Ok(())
    }
}
