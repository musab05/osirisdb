#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxnStatus {
    Active,
    Committed,
    Aborted,
}

pub struct Transaction {
    /// Unique monotonically increasing transaction ID.
    pub txn_id: u64,

    /// Current transaction state (Active, Committed, Aborted).
    pub status: TxnStatus,

    /// The LSN of the most recent log record written by this transaction.
    /// This forms the backward link (prev_lsn) for undo.
    pub last_lsn: u64,
}

impl Transaction {
    pub fn new(txn_id: u64) -> Self {
        Self {
            txn_id,
            status: TxnStatus::Active,
            last_lsn: 0,
        }
    }
}