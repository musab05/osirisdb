use std::{
    collections::HashMap,
    sync::{Arc, Mutex, atomic::AtomicU64},
};

use crate::storage::{log::log_manager::LogManager, txn::transaction::Transaction};

pub struct TransactionManager {
    /// Atomic generator for unique transaction IDs.
    next_txn_id: AtomicU64,

    /// Active transaction table (ATT) tracking in-flight transactions.
    active_txns: Mutex<HashMap<u64, Transaction>>,

    /// Shared global WAL log manager.
    log_manager: Arc<LogManager>,
}
