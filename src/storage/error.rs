use std::{path::PathBuf, write};

/// Errors that can occur during storage operations.
///
/// All variants carry enough context for the caller to produce
/// a useful error message without the storage layer needing to
/// know about the interner or symbol resolution.
#[derive(Debug, Clone, PartialEq)]
pub enum StorageError {
    /// A directory that was expected to already exist was not found.
    DirectoryNotFound(PathBuf),

    /// A directory already exists where we tried to create one.
    ///
    /// This should not happen in normal operation since the binder
    /// checks existence first — indicates a race condition or
    /// manual filesystem interference.
    DirectoryAlreadyExists(PathBuf),

    /// A filesystem operation failed.
    ///
    /// Carries the path that was being operated on and the
    /// OS-level error description.
    Io { path: PathBuf, reason: String },

    /// A page read/write was attempted with a page_id that does not exist.
    PageOutOfBounds { page_id: u32, num_pages: u32 },

    /// All buffer pool frames are pinned — no victim can be evicted.
    ///
    /// Caller must unpin at least one frame before requesting another page.
    BufferPoolFull,

    /// A value's type did not match the column's declared type,
    /// or the column count did not match the schema.
    TupleError(String),

    /// Duplicate key check
    DuplicateKey,

    /// index check
    IndexNotInitialized,

    /// Data or page layout corruption encountered.
    CorruptedData(String),

    /// A WAL logging or recovery error occurred.
    WalError(String),

    /// A thread panicked while holding a mutex lock.
    LockPoisoned(String),

    /// log records error
    LogRecordTooSmall,

    /// Checksum verification failed
    ChecksumMismatch,

    /// Invalid log record type
    InvalidRecordType,
}

impl StorageError {
    /// Wraps a `std::io::Error` with the path that caused it.
    pub(crate) fn io(path: impl Into<PathBuf>, e: std::io::Error) -> Self {
        StorageError::Io {
            path: path.into(),
            reason: e.to_string(),
        }
    }
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::DirectoryNotFound(p) => {
                write!(f, "directory not found: {}", p.display())
            }
            StorageError::DirectoryAlreadyExists(p) => {
                write!(f, "directory already exists: {}", p.display())
            }
            StorageError::Io { path, reason } => {
                write!(f, "I/O error at {}: {}", path.display(), reason)
            }
            StorageError::PageOutOfBounds { page_id, num_pages } => {
                write!(
                    f,
                    "page {} out of bounds (file has {} pages)",
                    page_id, num_pages
                )
            }
            StorageError::BufferPoolFull => {
                write!(f, "buffer pool is full: all frames are pinned")
            }
            StorageError::TupleError(msg) => {
                write!(f, "tuple error: {}", msg)
            }
            StorageError::DuplicateKey => {
                write!(f, "duplicate key violates unique constraint")
            }
            StorageError::IndexNotInitialized => {
                write!(f, "index has no pages yet")
            }
            StorageError::CorruptedData(msg) => {
                write!(f, "corrupted data: {}", msg)
            }
            StorageError::WalError(msg) => {
                write!(f, "WAL error: {}", msg)
            }
            StorageError::LockPoisoned(msg) => {
                write!(f, "lock poisoned: {}", msg)
            }
            StorageError::LogRecordTooSmall => {
                write!(f, "Record size is small")
            }
            StorageError::ChecksumMismatch => {
                write!(f, "checksum mismatch in log record")
            }
            StorageError::InvalidRecordType => {
                write!(f, "invalid record type encountered")
            }
        }
    }
}

impl std::error::Error for StorageError {}
