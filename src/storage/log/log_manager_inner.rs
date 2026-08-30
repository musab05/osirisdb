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

/// Ping-pong double buffer for non-blocking concurrent WAL appending.
pub struct DoubleBuffer {
    /// Active buffer that worker threads append into.
    pub active: Vec<u8>,
    /// Flush buffer currently being drained to disk by the flusher thread.
    pub flush: Vec<u8>,
}

/// Internal state shared between [`LogManager`] and the background flusher thread.
///
/// Wrapped in an [`std::sync::Arc`] to permit concurrent lock-free LSN generation,
/// buffered log appending, and background group-commit flushing.
pub struct LogManagerInner {
    /// The physical log file handle where WAL records are persisted.
    pub file: Mutex<File>,

    /// In-memory ping-pong double buffers.
    pub buffers: Mutex<DoubleBuffer>,

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

impl DoubleBuffer {
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.active, &mut self.flush);
    }
}
