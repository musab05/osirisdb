#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TableLockMode {
    AccessShare,
    RowShare,
    RowExclusive,
    Share,
    ShareRowExclusive,
    Exclusive,
    AccessExclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RowLockMode {
    KeyShare,
    Share,
    NoKeyUpdate,
    Update, //Exclusive
}

impl RowLockMode {
    pub fn is_compatible(&self, other: RowLockMode) -> bool {
        match (self, other) {
            (RowLockMode::KeyShare, RowLockMode::Update)
            | (RowLockMode::Update, RowLockMode::KeyShare) => false,

            (RowLockMode::Share, RowLockMode::NoKeyUpdate)
            | (RowLockMode::Share, RowLockMode::Update)
            | (RowLockMode::NoKeyUpdate, RowLockMode::Share)
            | (RowLockMode::Update, RowLockMode::Share) => false,

            (RowLockMode::NoKeyUpdate, RowLockMode::NoKeyUpdate)
            | (RowLockMode::NoKeyUpdate, RowLockMode::Update)
            | (RowLockMode::Update, RowLockMode::NoKeyUpdate) => false,
            (RowLockMode::Update, RowLockMode::Update) => false,

            _ => true,
        }
    }
}
