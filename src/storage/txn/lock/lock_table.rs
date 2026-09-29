use std::sync::Condvar;

use crate::storage::txn::lock::lock_mode::RowLockMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockStatus {
    Granted,
    Waiting,
}

#[derive(Debug, Clone)]
pub struct LockRequest {
    pub txn_id: u64,
    pub mode: RowLockMode,
    pub status: LockStatus,
}

pub struct LockHead {
    pub granted: Vec<LockRequest>,
    pub waiting: Vec<LockRequest>,
    pub condvar: Condvar,
}

impl LockHead {
    pub fn new() -> Self {
        Self {
            granted: Vec::new(),
            waiting: Vec::new(),
            condvar: Condvar::new(),
        }
    }

    /// Checks if a new request is compatible with all currently GRANTED requests.
    pub fn is_compatible(&self, req_mode: RowLockMode, requesting_txn: u64) -> bool {
        for g in &self.granted {
            if g.txn_id != requesting_txn && !g.mode.is_compatible(req_mode) {
                return false;
            }
        }
        true
    }
}
