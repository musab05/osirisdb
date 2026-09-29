use crate::storage::RecordId;

/// Identifies any lockable database object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LockResource {
    /// Table identified by its registered file_id (or table schema/name hash)
    Table(u32),
    /// Specific row identified by file_id and physical slot
    Row { file_id: u32, rid: RecordId },
}
