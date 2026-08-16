use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

use crate::storage::{StorageError, log::log_record::LogRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lsn(pub u64);

pub struct LogManager {
    /// The log file on disk where WAL records are persisted.
    file: File,

    /// In-memory buffer to hold serialized log records before they are flushed.
    /// We will flush this when it gets full, or when forced by the BufferPool.
    log_buffer: Vec<u8>,

    /// The next Log Sequence Number to assign to an incoming log record.
    next_lsn: AtomicU64,

    /// The highest LSN that has been safely flushed to disk.
    /// This is crucial for the Buffer Pool to enforce the WAL rule.
    flushed_lsn: AtomicU64,

    /// Maximum size of the log buffer (e.g., 4MB) before an automatic flush is triggered.
    buffer_capacity: usize,
}

impl LogManager {
    pub fn new(log_path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path.as_ref())
            .map_err(|e| StorageError::io(log_path.as_ref(), e))?;

        Ok(LogManager {
            file,
            log_buffer: Vec::with_capacity(4096),
            next_lsn: AtomicU64::new(1),
            flushed_lsn: AtomicU64::new(0),
            buffer_capacity: 4096,
        })
    }

    /// Appends a new log record to the in-memory log buffer.
    ///
    /// This method assigns a monotonically increasing LSN to the record.
    /// If the serialized record exceeds the remaining buffer capacity,
    /// the buffer is automatically flushed to disk before appending.
    pub fn append_record(&mut self, record: &mut LogRecord) -> Result<Lsn, StorageError> {
        // Atomically get and increment the LSN
        let assigned_lsn = self.next_lsn.fetch_add(1, Ordering::SeqCst);
        record.lsn = assigned_lsn;

        // Serialize the record
        let bytes = record.serialize();

        // Flush if buffer capacity exceeded
        if self.log_buffer.len() + bytes.len() > self.buffer_capacity {
            self.flush()?;
        }

        // Append to buffer
        self.log_buffer.extend_from_slice(&bytes);
        Ok(Lsn(assigned_lsn))
    }

    /// Flushes all pending log records in the memory buffer to the physical disk file.
    ///
    /// This method blocks until the operating system confirms the data is fully
    /// synced to the underlying storage device (via `fsync`). It then updates
    /// the `flushed_lsn` to reflect the newly persisted records.
    pub fn flush(&mut self) -> Result<(), StorageError> {
        if self.log_buffer.is_empty() {
            return Ok(());
        }

        self.file
            .write_all(&self.log_buffer)
            .map_err(|e| StorageError::io("log_file", e))?;

        self.file
            .sync_data()
            .map_err(|e| StorageError::io("log_file", e))?;

        // Here updating to work with Atomicu64
        let current_next = self.next_lsn.load(Ordering::SeqCst);
        self.flushed_lsn
            .store(current_next.saturating_sub(1), Ordering::SeqCst);

        self.log_buffer.clear();
        Ok(())
    }

    /// Returns the highest Log Sequence Number (LSN) that is safely written to disk.
    ///
    /// The Buffer Pool Manager uses this to enforce the Write-Ahead Logging (WAL) rule
    /// by ensuring a dirty page's `page_lsn` is less than or equal to this value
    /// before evicting the page to disk.
    pub fn get_flushed_lsn(&self) -> u64 {
        self.flushed_lsn.load(Ordering::SeqCst)
    }
}
