use crate::storage::{RecordId, StorageError};

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

impl TupleHeader {
    /// Creates a new `TupleHeader` for an inserted tuple.
    ///
    /// By default
    /// - `xmax = 0` (liev tuple)
    /// 0 `ctid` points to the tuple's own `RecordId`
    /// - `infomask = 0` (no hints set yet)
    pub fn new(xmin: u64, cid: u32, ctid: RecordId) -> Self {
        Self {
            xmin,
            xmax: 0,
            cid,
            ctid,
            infomask: 0,
        }
    }

    /// Returns `true` if this tuple is still active (not deleted or superseded by an update)
    #[inline]
    pub fn is_active(&self) -> bool {
        self.xmax == 0
    }

    /// Returns `true` if this tuple has deleted or superseded by an update
    #[inline]
    pub fn is_deleted(&self) -> bool {
        self.xmax != 0
    }

    /// Mark this tuple to a newer versin upon update
    pub fn mark_deleted(&mut self, xmax: u64) {
        self.xmax = xmax;
    }

    /// Chains this tuple to newer version upon update
    pub fn mark_update(&mut self, xmax: u64, new_cid: RecordId) {
        self.xmax = xmax;
        self.ctid = new_cid;
        self.infomask |= TupleInfoMask::UPDATED;
    }

    /// Serializes the header into a fixed 28-byte array.
    pub fn to_bytes(&self) -> [u8; TUPLE_HEADER_SIZE] {
        let mut bytes = [0u8; TUPLE_HEADER_SIZE];
        bytes[0..8].copy_from_slice(&self.xmin.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.xmax.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.cid.to_le_bytes());
        bytes[20..26].copy_from_slice(&self.ctid.to_bytes());
        bytes[26..28].copy_from_slice(&self.infomask.to_le_bytes());
        bytes
    }

    /// Deserializes a `TupleHeader` from a byte slice (must have at least 28 bytes).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, StorageError> {
        if bytes.len() < TUPLE_HEADER_SIZE {
            return Err(StorageError::TupleError(format!(
                "invalid TupleHeader length: expected at least {} bytes, found {}",
                TUPLE_HEADER_SIZE,
                bytes.len()
            )));
        }
        let xmin = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let xmax = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let cid = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        let ctid = RecordId::from_bytes(&bytes[20..26])?;
        let infomask = u16::from_le_bytes(bytes[26..28].try_into().unwrap());
        Ok(Self {
            xmin,
            xmax,
            cid,
            ctid,
            infomask,
        })
    }
}
