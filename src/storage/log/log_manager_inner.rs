//! Internal shared state for [`LogManager`].
//!
//! Houses all synchronization primitives (`Mutex`, `Condvar`, `Atomic*`)
//! shared between concurrent foreground database worker threads and the
//! background WAL flusher thread.

use std::{
    fs::File,
    sync::{
        Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64},
    },
};

/// Internal state shared between [`LogManager`] and the background flusher thread.
///
/// Wrapped in an [`std::sync::Arc`] to permit concurrent lock-free LSN generation,
/// buffered log appending, and background group-commit flushing.
pub struct LogManagerInner {
    /// The physical log file handle where WAL records are persisted.
    pub file: Mutex<File>,

    /// In-memory buffer storing serialized log records pending an `fsync`.
    pub log_buffer: Mutex<Vec<u8>>,

    /// Maximum capacity of `log_buffer` in bytes before forcing a flush.
    pub buffer_capacity: usize,

    /// Atomic Log Sequence Number generator for strictly monotonic, lock-free LSN allocation.
    pub next_lsn: AtomicU64,

    /// The highest LSN safely persisted to disk, paired with a [`Condvar`]
    /// to notify committing transactions during group commit.
    pub flushed_state: (Mutex<u64>, Condvar),

    /// Atomic flag indicating whether the background flusher thread should continue running.
    pub is_running: AtomicBool,
}
