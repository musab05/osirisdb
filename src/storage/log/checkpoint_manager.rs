use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    vec,
};

use crate::storage::{
    LogManager, StorageError, TransactionManager,
    log::{
        checkpoint_data::CheckpointData,
        log_record::{LogRecord, RecordType},
    },
};

pub struct CheckpointManager {
    log_manager: Arc<LogManager>,
    txn_manager: Arc<TransactionManager>,
    meta_path: PathBuf,
}

impl CheckpointManager {
    pub fn new(
        log_manager: Arc<LogManager>,
        txn_manager: Arc<TransactionManager>,
        meta_path: impl AsRef<Path>,
    ) -> Self {
        Self {
            log_manager,
            txn_manager,
            meta_path: meta_path.as_ref().to_path_buf(),
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
        let data = CheckpointData {
            active_txns,
            dirty_pages: vec![],
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

        Ok(begin_lsn.0)
    }
}
