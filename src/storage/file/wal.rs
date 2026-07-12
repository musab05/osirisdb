use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use crate::storage::{
    error::StorageError,
    page::{TablePage, table_page::PAGE_SIZE},
};

/// Marks the start of a valid record. Used to detect a torn/garbage
/// record after a crash — if the magic doesn't match, everything from
/// that point on is untrustworthy and recovery stops there.
const WAL_MAGIC: u32 = 0x57414C31; // "WAL1"

/// magic(4) + page_id(4) + page data(PAGE_SIZE) + checksum(4)
const RECORD_SIZE: usize = 4 + 4 + PAGE_SIZE + 4;

/// A minimal physical (redo-only) write-ahead log for a single data file.
///
/// One `Wal` lives alongside one `HeapFile` — every `.dat`/`.idx` file
/// gets its own `<path>.wal` sibling. This is *not* a transaction log:
/// there is no BEGIN/COMMIT concept here, no undo. Its only job is
/// crash-consistency for individual page writes — if the process dies
/// mid-write to the real file, the WAL lets `HeapFile::open` detect and
/// reapply the last known-good version of that page on restart.
///
/// # Why log the whole page instead of a diff
///
/// Physical (whole-page) logging is the simplest correct approach and
/// matches how pages are already written elsewhere in this engine (no
/// existing diff/patch format to log against). The cost is one extra
/// full-page write per page write — acceptable for now; a leaner
/// physiological log (log only the changed bytes) is a later
/// optimization, not a correctness requirement.
pub struct Wal {
    file: File,
    path: PathBuf,
}

impl Wal {
    /// Derives the WAL path for a given data file path by appending
    /// `.wal` to the full filename (e.g. `users.dat` → `users.dat.wal`).
    fn wal_path_for(data_path: &Path) -> PathBuf {
        let mut os = data_path.as_os_str().to_owned();
        os.push(".wal");
        PathBuf::from(os)
    }

    /// Opens (or creates) the WAL file sibling to `data_path`.
    pub fn open(data_path: &Path) -> Result<Self, StorageError> {
        let path = Self::wal_path_for(data_path);
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .map_err(|e| StorageError::io(&path, e))?;
        Ok(Self { file, path })
    }

    /// Appends a full-page-image record and fsyncs it before returning.
    ///
    /// This fsync is the actual "write-ahead" guarantee: by the time
    /// this returns, the record is durable on disk, so the caller can
    /// safely proceed to write the real page to its actual file knowing
    /// that if the process dies mid-write, this record survives to
    /// describe what the page should have looked like.
    pub fn log_page(&mut self, page_id: u32, page: &TablePage) -> Result<(), StorageError> {
        self.file
            .seek(SeekFrom::End(0))
            .map_err(|e| StorageError::io(&self.path, e))?;

        let mut buf = Vec::with_capacity(RECORD_SIZE);
        buf.extend_from_slice(&WAL_MAGIC.to_le_bytes());
        buf.extend_from_slice(&page_id.to_le_bytes());
        buf.extend_from_slice(page.as_bytes());

        // Checksum covers page_id + page data (not the magic — the magic
        // is the record-boundary marker, not payload).
        let checksum = fnv1a(&buf[4..]);
        buf.extend_from_slice(&checksum.to_le_bytes());

        self.file
            .write_all(&buf)
            .map_err(|e| StorageError::io(&self.path, e))?;

        // Real fsync — not the no-op flush() used elsewhere in this
        // engine. This is the one place durability actually matters.
        self.file
            .sync_data()
            .map_err(|e| StorageError::io(&self.path, e))?;

        Ok(())
    }

    /// Scans the WAL from the start and returns every fully-valid
    /// record (correct magic + checksum), in order.
    ///
    /// Stops at the first invalid or incomplete record. Because this is
    /// an append-only log, a torn record can only ever occur at the very
    /// end (the tail of an interrupted write) — everything before it is
    /// trustworthy, everything from it onward is discarded.
    pub fn read_all_valid_records(&mut self) -> Result<Vec<(u32, [u8; PAGE_SIZE])>, StorageError> {
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|e| StorageError::io(&self.path, e))?;

        let mut records = Vec::new();
        let mut buf = vec![0u8; RECORD_SIZE];

        loop {
            if self.file.read_exact(&mut buf).is_err() {
                break; // EOF or short read at the tail — nothing more to recover
            }

            let magic = u32::from_le_bytes(buf[0..4].try_into().unwrap());
            if magic != WAL_MAGIC {
                break;
            }

            let page_id = u32::from_le_bytes(buf[4..8].try_into().unwrap());
            let data = &buf[8..8 + PAGE_SIZE];
            let stored_checksum =
                u32::from_le_bytes(buf[8 + PAGE_SIZE..RECORD_SIZE].try_into().unwrap());
            let computed = fnv1a(&buf[4..8 + PAGE_SIZE]);

            if computed != stored_checksum {
                break; // torn or corrupted — stop, discard this and everything after
            }

            let mut page_data = [0u8; PAGE_SIZE];
            page_data.copy_from_slice(data);
            records.push((page_id, page_data));
        }

        Ok(records)
    }

    /// Truncates the WAL. Call only once every record in it has been
    /// durably applied to the real data file (fsynced) — at that point
    /// the WAL's job is done and replaying it again would be redundant.
    pub fn checkpoint(&mut self) -> Result<(), StorageError> {
        self.file
            .set_len(0)
            .map_err(|e| StorageError::io(&self.path, e))?;
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|e| StorageError::io(&self.path, e))?;
        Ok(())
    }
}

/// FNV-1a 32-bit — not cryptographic, just here to catch torn writes
/// and bit-level corruption. No external crate needed.
fn fnv1a(data: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c9dc5;
    for &b in data {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}
