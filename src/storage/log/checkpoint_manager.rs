use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    vec,
};

use crate::storage::{
    BufferPool, LogManager, StorageError, TransactionManager,
    log::{
        checkpoint_data::CheckpointData,
        log_record::{LogRecord, RecordType},
    },
};

pub struct CheckpointManager {
    log_manager: Arc<LogManager>,
    txn_manager: Arc<TransactionManager>,
    meta_path: PathBuf,
    buffer_pool: Arc<Mutex<BufferPool>>,
}

impl CheckpointManager {
    pub fn new(
        log_manager: Arc<LogManager>,
        txn_manager: Arc<TransactionManager>,
        meta_path: impl AsRef<Path>,
        buffer_pool: Arc<Mutex<BufferPool>>,
    ) -> Self {
        Self {
            log_manager,
            txn_manager,
            meta_path: meta_path.as_ref().to_path_buf(),
            buffer_pool,
        }
    }

    pub fn checkpoint(&self) -> Result<u64, StorageError> {
        // Write CheckpointBegin
        let mut begin_record = LogRecord {
            lsn: 0,
            prev_lsn: 0,
            txt_id: 0,
            record_type: RecordType::CheckpointBegin,
            file_id: 0,
            page_id: 0,
            offset: 0,
            length: 0,
            before_image: vec![],
            after_image: vec![],
        };

        let begin_lsn = self.log_manager.append_record(&mut begin_record)?;

        // Snapshot active transactions from txn_manager
        let active_txns = self.txn_manager.get_active_transactions();

        // Snapshot dirt pages from the buffer pool
        let bp = self.buffer_pool.lock().unwrap();
        let dirty_page_keys = bp.get_dirty_pages();
        drop(bp); // release lock quickly

        let dirty_pages: Vec<((u32, u32), u64)> = dirty_page_keys
            .into_iter()
            .map(|(fid, pid)| ((fid, pid), begin_lsn.0))
            .collect();

        let data = CheckpointData {
            active_txns,
            dirty_pages,
        };

        // Write checkpoint with serialized data after_iamge
        let mut end_record = LogRecord {
            lsn: 0,
            prev_lsn: 0,
            txt_id: 0,
            record_type: RecordType::CheckpointEnd,
            file_id: 0,
            page_id: 0,
            offset: 0,
            length: 0,
            before_image: vec![],
            after_image: data.serialize(),
        };
        self.log_manager.append_record(&mut end_record)?;

        // Flush wal to disk for durability
        self.log_manager.flush()?;

        // Write master record
        fs::write(&self.meta_path, begin_lsn.0.to_le_bytes())
            .map_err(|e| StorageError::io(&self.meta_path, e))?;

        // truncate WAL: discard records before this checkpoint
        self.log_manager.truncate_before(begin_lsn.0)?;

        Ok(begin_lsn.0)
    }
}
