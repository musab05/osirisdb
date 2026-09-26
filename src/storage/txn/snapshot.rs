#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// transactios with id < xmin are all completed (visible if committed)
    pub xmin: u64,
    /// Transactions with id >= xmax are in the future (invisible)
    pub xmax: u64,
    /// Active transaction IDs between xmin and xmax at the time the snapshot was taken
    pub xip_list: Vec<u64>,
}

impl Snapshot {
    /// Return true if `txn_id` was active/in-flight when the snapshot was taken
    pub fn is_active(&self, txn_id: u64) -> bool {
        if txn_id < self.xmin {
            false
        } else if txn_id >= self.xmax {
            true // considered in-flight / future
        } else {
            self.xip_list.contains(&txn_id)
        }
    }
}
