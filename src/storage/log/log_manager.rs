//! Write-Ahead Log (WAL) Manager with Group Commit support.
//!
//! The [`LogManager`] coordinates all WAL record appending, atomic LSN allocation,
//! asynchronous group commits, and disk synchronization for the OsirisDB storage engine.

use std::{
    fs::OpenOptions,
    io::Write,
    mem::take,
    path::Path,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time,
};

use crate::storage::{
    StorageError,
    log::{log_manager_inner::LogManagerInner, log_record::LogRecord},
};

/// Strongly typed wrapper around a 64-bit Log Sequence Number (LSN).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lsn(pub u64);

/// Coordinates the Write-Ahead Log (WAL) for transactions and buffer pool synchronization.
///
/// `LogManager` is thread-safe and supports high-throughput concurrent logging via:
/// 1. **Lock-free LSN generation:** Atomic CPU instructions allocate unique monotonic LSNs.
/// 2. **Group Commit:** Committing transactions sleep on a condition variable until a background
///    thread batches pending writes into a single `fsync`.
pub struct LogManager {
    /// Shared state across caller threads and background flusher thread.
    inner: Arc<LogManagerInner>,

    /// Handle to the background flusher thread (joined cleanly on drop).
    flusher_handle: Option<JoinHandle<()>>,
}

impl LogManager {
    /// Opens or creates a WAL file at `log_path` and spawns the background flusher thread.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Io`] if the log file cannot be opened or created.
    pub fn new(log_path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path.as_ref())
            .map_err(|e| StorageError::io(log_path.as_ref(), e))?;

        let inner = Arc::new(LogManagerInner {
            file: Mutex::new(file),
            log_buffer: Mutex::new(Vec::with_capacity(4096)),
            buffer_capacity: 4096,
            next_lsn: AtomicU64::new(1),
            flushed_state: (Mutex::new(0), Condvar::new()),
            is_running: AtomicBool::new(true),
        });

        let inner_clone = Arc::clone(&inner);
        let flusher_handle = thread::spawn(move || {
            Self::flusher_loop(inner_clone);
        });

        Ok(LogManager {
            inner,
            flusher_handle: Some(flusher_handle),
        })
    }

    /// Appends a new log record to the in-memory log buffer and assigns it a unique LSN.
    ///
    /// This method is thread-safe. If appending causes the buffer to exceed its capacity,
    /// a synchronous flush is triggered to free space.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError`] if an automatic buffer flush fails.
    pub fn append_record(&self, record: &mut LogRecord) -> Result<Lsn, StorageError> {
        // Atomically get and increment the LSN
        let assigned_lsn = self.inner.next_lsn.fetch_add(1, Ordering::SeqCst);
        record.lsn = assigned_lsn;

        // Serialize the record
        let bytes = record.serialize();

        // Append to inner buffer under lock
        let mut buffer = self.inner.log_buffer.lock().unwrap();
        buffer.extend_from_slice(&bytes);

        // If buffer capacity is exceeded, flush immediately
        if buffer.len() >= self.inner.buffer_capacity {
            drop(buffer); // Unlock before flushing
            self.flush()?;
        }

        Ok(Lsn(assigned_lsn))
    }

    /// Flushes all pending log records in the memory buffer to the physical disk file.
    ///
    /// Blocks until the operating system confirms data is written and synced via `fsync`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Io`] if disk write or `fsync` fails.
    pub fn flush(&self) -> Result<(), StorageError> {
        Self::flush_internal(&self.inner)
    }

    /// Internal helper that drains the in-memory buffer, syncs to disk, and notifies waiting threads.
    fn flush_internal(inner: &LogManagerInner) -> Result<(), StorageError> {
        // 1. Swap/drain the buffer under lock
        let mut buffer_guard = inner.log_buffer.lock().unwrap();
        if buffer_guard.is_empty() {
            return Ok(());
        }

        let pending_bytes = take(&mut *buffer_guard);
        // Compute the highest LSN covered by this batch
        let current_next = inner.next_lsn.load(Ordering::SeqCst);
        let current_flushing_lsn = current_next.saturating_sub(1);
        drop(buffer_guard); // Release buffer lock early

        // 2. Write to disk and fsync
        let mut file_guard = inner.file.lock().unwrap();
        file_guard
            .write_all(&pending_bytes)
            .map_err(|e| StorageError::io("log_file", e))?;
        file_guard
            .sync_data()
            .map_err(|e| StorageError::io("log_file", e))?;
        drop(file_guard);

        // 3. Update flushed_lsn and wake up all waiting transactions (Group Commit)
        let (lock, cvar) = &inner.flushed_state;
        let mut flushed_lsn = lock.lock().unwrap();
        *flushed_lsn = current_flushing_lsn;
        cvar.notify_all(); // Wakes up any thread blocked in wait_for_flush

        Ok(())
    }

    /// Returns the highest Log Sequence Number (LSN) that is safely written and synced to disk.
    ///
    /// The Buffer Pool Manager uses this to enforce the Write-Ahead Logging (WAL) rule
    /// ensuring a dirty page's `page_lsn <= flushed_lsn` before evicting the page.
    pub fn get_flushed_lsn(&self) -> u64 {
        let (lock, _) = &self.inner.flushed_state;
        *lock.lock().unwrap()
    }

    /// Blocks the calling thread until the given `target_lsn` has been fsynced to disk.
    ///
    /// Used by committing transactions to participate in Group Commit without performing
    /// redundant `fsync` calls.
    pub fn wait_for_flush(&self, target_lsn: u64) -> Result<(), StorageError> {
        let (lock, cvar) = &self.inner.flushed_state;
        let mut flushed_lsn = lock.lock().unwrap();

        // Loop until the background flusher advances flushed_lsn >= target_lsn
        while *flushed_lsn < target_lsn {
            flushed_lsn = cvar.wait(flushed_lsn).unwrap();
        }

        Ok(())
    }

    /// Continuous background loop that flushes pending records every 5 milliseconds.
    fn flusher_loop(inner: Arc<LogManagerInner>) {
        while inner.is_running.load(Ordering::Relaxed) {
            thread::sleep(time::Duration::from_millis(5)); // Batch flush interval
            let _ = Self::flush_internal(&inner);
        }
        // Final flush on database shutdown
        let _ = Self::flush_internal(&inner);
    }
}

impl Drop for LogManager {
    /// Signals the background flusher thread to exit and joins it before destruction.
    fn drop(&mut self) {
        self.inner.is_running.store(false, Ordering::Relaxed);
        if let Some(handle) = self.flusher_handle.take() {
            let _ = handle.join();
        }
    }
}
