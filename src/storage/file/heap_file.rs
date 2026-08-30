use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

use crate::storage::{
    error::StorageError,
    page::{TablePage, table_page::PAGE_SIZE},
};

/// A disk-backed sequence of fixed-size pages for one table.
///
/// A `HeapFile` wraps a single `std::fs::File` and treats it as a flat
/// array of `PAGE_SIZE`-byte blocks, indexed by a `u32` page id:
///
/// ```text
/// byte offset 0                    → page 0
/// byte offset PAGE_SIZE            → page 1
/// byte offset 2 * PAGE_SIZE        → page 2
/// ...
/// byte offset n * PAGE_SIZE        → page n
/// ```
///
/// This is the simplest possible layout: no file header, no metadata block,
/// just pages back-to-back. `num_pages` is computed from the file length at
/// open time and updated as pages are allocated, so no separate metadata
/// file is needed.
///
/// # No buffer pool
///
/// Every `read_page`/`write_page` call goes directly to the OS via
/// `seek` + `read_exact` / `write_all`. There is no caching. The buffer
/// pool (Stage 3) will sit in front of `HeapFile` and avoid redundant I/O
/// by keeping frequently-used pages in memory.
///
/// # One `HeapFile` per table
///
/// Each table's data lives in its own `.dat` file, created by
/// [`crate::storage::Storage::create_table_file`]. The path passed to
/// [`HeapFile::open`] should point to that file.
pub struct HeapFile {
    /// The open file handle — kept open for the lifetime of the struct.
    file: File,

    /// Path to the file — stored for error messages only.
    path: PathBuf,

    /// Number of pages currently in this file.
    ///
    /// Computed from `file.metadata().len() / PAGE_SIZE` at open time,
    /// then incremented by [`Self::allocate_page`] as new pages are added.
    /// Never decremented (page deallocation is not implemented in Stage 2).
    pub num_pages: u32,
}

impl HeapFile {
    /// Opens (or creates) the heap file at `path`.
    ///
    /// If the file does not exist it is created empty (`num_pages = 0`).
    /// If it already exists, `num_pages` is computed from its length.
    ///
    /// # Partial-page detection
    ///
    /// If the file length is not an exact multiple of `PAGE_SIZE`, the
    /// trailing partial block is silently ignored — `num_pages` is
    /// computed by integer division (`len / PAGE_SIZE`). This can happen
    /// if a previous write was interrupted mid-page; the global RecoveryEngine
    ///  handles crash recovery via the centralized WAL.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Io`] if the file cannot be opened or its
    /// metadata cannot be read.
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let path = path.into();

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true) // create if not exists; no-op if already exists
            .open(&path)
            .map_err(|e| StorageError::io(&path, e))?;

        let file_len = file
            .metadata()
            .map_err(|e| StorageError::io(&path, e))?
            .len();

        // Integer division: any partial trailing block is ignored.
        let num_pages = (file_len / PAGE_SIZE as u64) as u32;

        let me = Self {
            file,
            path,
            num_pages,
        };

        Ok(me)
    }

    /// Allocates a new page at the end of the file and returns its `page_id`.
    ///
    /// Writes a zero-initialized [`Page`] to disk immediately so that the
    /// on-disk file length always equals `num_pages * PAGE_SIZE`. This
    /// avoids holes that would make `read_page`'s bounds check lie.
    ///
    /// The first call on a fresh file returns `0`, the second `1`, and so on.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Io`] if the seek or write fails.
    ///
    /// Allocates a new page at the end of the file and returns its `page_id`.
    pub fn allocate_page(&mut self) -> Result<u32, StorageError> {
        let page_id = self.num_pages;
        let page = TablePage::new(page_id);

        self.seek_to(page_id)?;
        self.file
            .write_all(page.as_bytes())
            .map_err(|e| StorageError::io(&self.path, e))?;

        self.num_pages += 1;
        Ok(page_id)
    }

    /// Reads the page at `page_id` from disk and returns it.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::PageOutOfBounds`] if `page_id >= num_pages`.
    /// Returns [`StorageError::Io`] if the seek or read fails.
    pub fn read_page(&mut self, page_id: u32) -> Result<TablePage, StorageError> {
        if page_id >= self.num_pages {
            return Err(StorageError::PageOutOfBounds {
                page_id,
                num_pages: self.num_pages,
            });
        }

        self.seek_to(page_id)?;

        // `read_exact` errors if the OS returns fewer bytes than requested.
        // This should never happen since page_id < num_pages guarantees
        // the bytes were written by `allocate_page`, but it is a correct
        // safety net against filesystem-level corruption.
        let mut buf = [0u8; PAGE_SIZE];
        self.file
            .read_exact(&mut buf)
            .map_err(|e| StorageError::io(&self.path, e))?;

        Ok(TablePage::from_bytes(buf))
    }

    /// Writes `page` to its position in the data file.
    ///
    /// # Durability Note
    ///
    /// The write goes to the OS page cache without an immediate `fsync`.
    /// Durability is ensured by the global [`LogManager`] WAL — the
    /// [`BufferPool`] enforces `page_lsn <= flushed_lsn` before any
    /// dirty page hits disk. Call [`Self::sync()`] to force an `fsync`.
    pub fn write_page(&mut self, page_id: u32, page: &TablePage) -> Result<(), StorageError> {
        if page_id >= self.num_pages {
            return Err(StorageError::PageOutOfBounds {
                page_id,
                num_pages: self.num_pages,
            });
        }

        self.seek_to(page_id)?;
        self.file
            .write_all(page.as_bytes())
            .map_err(|e| StorageError::io(&self.path, e))?;

        Ok(())
    }

    /// Writes `page` directly to disk and immediately flushes changes to stable storage (`fsync`).
    ///
    /// Use this variant for WAL-bypassing operations (e.g., bulk loading or unlogged table writes)
    /// where WAL replay will not be available to recover un-synced page cache writes.
    pub fn write_page_durable(
        &mut self,
        page_id: u32,
        page: &TablePage,
    ) -> Result<(), StorageError> {
        self.write_page(page_id, page)?;
        self.file
            .sync_all()
            .map_err(|e| StorageError::io(&self.path, e))?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────
    // Internal helpers
    // ─────────────────────────────────────────────────────────────────

    /// Seeks the internal file cursor to the byte offset of `page_id`.
    fn seek_to(&mut self, page_id: u32) -> Result<(), StorageError> {
        let offset = page_id as u64 * PAGE_SIZE as u64;
        self.file
            .seek(SeekFrom::Start(offset))
            .map_err(|e| StorageError::io(&self.path, e))?;
        Ok(())
    }

    /// Flushes all pending writes to stable storage (`fsync`).
    ///
    /// Call after a batch of dirty pages has been written (e.g. at the
    /// end of `BufferPool::flush_all`) to ensure the data file is
    /// durable on disk.
    pub fn sync(&mut self) -> Result<(), StorageError> {
        self.file
            .sync_all()
            .map_err(|e| StorageError::io(&self.path, e))?;
        Ok(())
    }
}
