use crate::storage::RecordId;

/// Total fixed byte size of a serialized TupleHeader on disk.
pub const TUPLE_HEADER_SIZE: usize = 28;

/// Status and hint bitflag stored in `TupleHeader::infomask`.
///
/// Follows the same idiom as `PageFlags` in `src/storage/page/header.rs`
pub struct TupleInfoMask;

impl TupleInfoMask {
    /// Tuple has one or more NULL attributes.
    pub const HAS_NULL: u16 = 1 << 0;
    /// Tuple contains variable-width attributes.
    pub const HAS_VARWIDTH: u16 = 1 << 1;
    /// Tuple contains out-of-line TOAST pointers.
    pub const HAS_TOAST: u16 = 1 << 2;
    /// Hint bit: `xmin` is known to have committed.
    pub const XMIN_COMMITTED: u16 = 1 << 3;
    /// Hint bit: `xmin` is known to have aborted / is invalid.
    pub const XMIN_ABORTED: u16 = 1 << 4;
    /// Hint bit: `xmax` is known to have committed.
    pub const XMAX_COMMITTED: u16 = 1 << 5;
    /// Hint bit: `xmax` is known to have aborted / is invalid.
    pub const XMAX_ABORTED: u16 = 1 << 6;
    /// Tuple has been updated and superseded by a newer version (ctid points to next version).
    pub const UPDATED: u16 = 1 << 7;

    /// Checks if a specific bit flag is active.
    #[inline]
    pub fn is_seet(mask: u16, flag: u16) -> bool {
        (mask & flag) != 0
    }
}

/// On-disk MVCC tuple header prepended to every physical heap tuple.
///
/// Enables Multi-Version Concurrency Control (MVCC), visibility resolution,
/// and update version chaining.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TupleHeader {
    /// Transaction ID that inserted/created this tuple
    pub xmin: u64,
    /// Transation ID that deleted or suprseded this tuple (0 if active/live)
    pub xmax: u64,
    /// Command ID within the transaction that created this tuple.
    pub cid: u32,
    /// Point to the tuple itself, or the next version if updated.
    pub ctid: RecordId,
    /// Status flags and commit/abort hint bits.
    pub infomask: u16,
}
