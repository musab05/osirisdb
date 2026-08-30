use std::{
    collections::{HashMap, HashSet},
    format, vec,
};

use crate::storage::{
    error::StorageError, file::HeapFile, log::log_manager::LogManager, page::TablePage,
};

/// A fixed-capacity in-memory cache of [`Page`]s backed by a [`HeapFile`].
///
/// # Why a buffer pool exists
///
/// Disk I/O is orders of magnitude slower than memory access. Without a
/// buffer pool every tuple read/write would hit disk. The buffer pool keeps
/// frequently accessed pages in memory ("frames") and only goes to disk on
/// a cache miss or when a dirty (modified) page must be evicted to free a
/// frame for a new page.
///
/// # Frames
///
/// The pool owns a fixed array of `capacity` slots called *frames*
/// (`frames: Vec<Option<Page>>`). Each frame either holds a cached `Page`
/// or is empty (`None`). Every cached page is tracked in `page_table`
/// which maps `page_id → frame_index` for O(1) cache lookups.
///
/// # Pin / Unpin protocol
///
/// Before reading or writing a page the caller **pins** it
/// (`pin_page` / `new_page`), which loads it into a frame if necessary
/// and increments the frame's `pin_count`. While `pin_count > 0` the
/// frame is *in use* and cannot be evicted.
///
/// After finishing with the page the caller **unpins** it (`unpin_page`),
/// decrementing the count and optionally marking the frame *dirty*. Only
/// frames with `pin_count == 0` are candidates for eviction.
///
/// # Eviction (LRU)
///
/// When all frames are occupied and a new page must be loaded, the pool
/// picks a *victim* frame — the unpinned frame with the smallest
/// `last_used` timestamp (Least Recently Used). If the victim is dirty
/// it is written back to disk before the frame is reused.
///
/// # Ownership
///
/// `BufferPool` owns the `HeapFile` and is the **only** component that
/// should call `HeapFile::read_page` / `write_page` directly. All higher
/// layers (tuple scanner, `INSERT` executor) go through the buffer pool.
pub struct BufferPool {
    /// The backing file — all disk reads and writes go through here.
    heap_files: HashMap<u32, HeapFile>, // Instead of separtely opening file for each here we have shared buffer all

    /// The frame array — each slot holds one cached page or is empty.
    frames: Vec<Option<TablePage>>,

    /// Maps `page_id → frame_index` for O(1) cache-hit detection.
    page_table: HashMap<(u32, u32), usize>, // (file_id, page_id) -> frame_index

    /// Maps frame_index -> page_id.
    frame_to_page: Vec<Option<(u32, u32)>>, // frame_index -> (file_id, page_id)

    /// Number of active pinners per frame.
    ///
    /// A frame with `pin_count > 0` is in use and cannot be evicted.
    /// Incremented by `pin_page`, decremented by `unpin_page`.
    pin_count: Vec<u32>,

    /// Whether the page in each frame has been modified since it was
    /// loaded from disk.
    ///
    /// Set to `true` by `get_page_mut` and by `unpin_page(dirty: true)`.
    /// Cleared to `false` after the page is written back to disk.
    dirty_flag: Vec<bool>,

    /// Reference bit per frame slot — set to `true` on access.
    referenced: Vec<bool>,

    /// Pointer to current position in the circular frame buffer.
    clock_hand: usize,

    /// Maximum number of frames this pool can hold simultaneously.
    capacity: usize,
}

impl BufferPool {
    /// Creates a new `BufferPool` wrapping `heap_file` with `capacity` frames.
    ///
    /// All frames start empty. No pages are loaded from disk until the first
    /// call to [`Self::pin_page`] or [`Self::new_page`].
    pub fn new(capacity: usize) -> Self {
        Self {
            heap_files: HashMap::new(),
            frames: (0..capacity).map(|_| None).collect(),
            page_table: HashMap::new(),
            frame_to_page: vec![None; capacity],
            pin_count: vec![0; capacity],
            dirty_flag: vec![false; capacity],
            referenced: vec![false; capacity],
            clock_hand: 0,
            capacity,
        }
    }

    // ─────────────────────────────────────────────────────────────────
    // Public API
    // ─────────────────────────────────────────────────────────────────

    /// Pins `page_id` into a frame and returns the frame index.
    ///
    /// If the page is already cached (cache hit) the existing frame is
    /// reused — no disk I/O occurs.
    ///
    /// If the page is not cached (cache miss) a victim frame is selected,
    /// evicted (written to disk if dirty), and the requested page is loaded
    /// from disk into that frame.
    ///
    /// The caller **must** call [`Self::unpin_page`] when done to allow
    /// the frame to be evicted in the future.
    ///
    /// # Errors
    ///
    /// - [`StorageError::PageOutOfBounds`] if `page_id >= num_pages`.
    /// - [`StorageError::BufferPoolFull`] if all frames are pinned and no
    ///   victim can be found.
    /// - [`StorageError::Io`] if the disk read or dirty-page write-back fails.
    pub fn pin_page(&mut self, file_id: u32, page_id: u32) -> Result<usize, StorageError> {
        // ── Cache hit ────────────────────────────────────────────────
        // The page is already in a frame — just bump its pin count and
        // update the LRU timestamp; no disk access needed.
        if let Some(&frame_id) = self.page_table.get(&(file_id, page_id)) {
            self.referenced[frame_id] = true;
            self.pin_count[frame_id] += 1;
            return Ok(frame_id);
        }

        // ── Cache miss ───────────────────────────────────────────────
        // Find a frame to load the page into.
        let frame_id = self.find_or_evict()?;
        let heap_file = self
            .heap_files
            .get_mut(&file_id)
            .ok_or(StorageError::UnknownFile(file_id))?;

        // Load the page from disk into the chosen frame.
        let mut page = heap_file.read_page(page_id)?;

        // Verify page integrity — a failed checksum means on-disk corruption.
        // Skip verification for fresh pages (checksum == 0 means never written with checksums).
        if page.checksum() != 0 && !page.verify_checksum() {
            return Err(StorageError::CorruptedData(format!(
                "page {} failed CRC32C checksum verification",
                page_id
            )));
        }
        self.frames[frame_id] = Some(page);

        // Register in the page table and mark as pinned.
        self.page_table.insert((file_id, page_id), frame_id);
        self.frame_to_page[frame_id] = Some((file_id, page_id));
        self.referenced[frame_id] = true;
        self.pin_count[frame_id] = 1;
        self.dirty_flag[frame_id] = false;

        Ok(frame_id)
    }

    /// Allocates a new page on disk, pins it into a frame, and returns
    /// `(page_id, frame_id)`.
    ///
    /// Use this when inserting into a table and the current last page is
    /// full — it extends the heap file with a fresh empty page.
    ///
    /// The caller **must** call [`Self::unpin_page`] when done.
    ///
    /// # Errors
    ///
    /// - [`StorageError::BufferPoolFull`] if all frames are pinned.
    /// - [`StorageError::Io`] if the page cannot be written to disk.
    pub fn new_page(&mut self, file_id: u32) -> Result<(u32, usize), StorageError> {
        // Find frame first
        let frame_id = self.find_or_evict()?;

        // Allocate the page on disk first so `num_pages` is up to date
        // before `pin_page` checks bounds.
        let heap_file = self
            .heap_files
            .get_mut(&file_id)
            .ok_or(StorageError::UnknownFile(file_id))?;

        let page_id = heap_file.allocate_page()?;
        let page = heap_file.read_page(page_id)?;

        self.frames[frame_id] = Some(page);
        self.page_table.insert((file_id, page_id), frame_id);
        self.frame_to_page[frame_id] = Some((file_id, page_id));
        self.referenced[frame_id] = true;
        self.pin_count[frame_id] = 1;
        self.dirty_flag[frame_id] = false;

        Ok((page_id, frame_id))
    }

    /// Returns a shared reference to the page stored in `frame_id`.
    ///
    /// # Panics
    ///
    /// Panics if `frame_id` is out of range or the frame is empty (i.e.
    /// the caller forgot to pin the page first).
    pub fn get_page(&self, frame_id: usize) -> &TablePage {
        self.frames[frame_id]
            .as_ref()
            .expect("frame is empty — did you forget to pin the page?")
    }

    /// Returns a mutable reference to the page stored in `frame_id` and
    /// marks the frame as dirty.
    ///
    /// The dirty flag ensures the modified page is written back to disk
    /// before the frame is evicted, even if the caller forgets to pass
    /// `dirty: true` to [`Self::unpin_page`].
    ///
    /// # Panics
    ///
    /// Panics if `frame_id` is out of range or the frame is empty.
    pub fn get_page_mut(&mut self, frame_id: usize) -> &mut TablePage {
        // Mark dirty immediately — the caller is about to modify the page.
        self.dirty_flag[frame_id] = true;
        self.frames[frame_id]
            .as_mut()
            .expect("frame is empty — did you forget to pin the page?")
    }

    /// Decrements the pin count of `frame_id`.
    ///
    /// If `dirty` is `true`, also marks the frame dirty so the page will
    /// be written back to disk before eviction. (Note: `get_page_mut`
    /// already sets the dirty flag automatically — passing `dirty: true`
    /// here is only strictly necessary if the caller obtained a mutable
    /// reference by other means.)
    ///
    /// # Panics
    ///
    /// Panics if `frame_id` is out of range or the pin count is already 0
    /// (double-unpin indicates a caller bug).
    pub fn unpin_page(&mut self, frame_id: usize, dirty: bool) {
        assert!(
            self.pin_count[frame_id] > 0,
            "unpin_page called on frame {} with pin_count == 0 (double unpin?)",
            frame_id
        );
        self.pin_count[frame_id] -= 1;
        if dirty {
            self.dirty_flag[frame_id] = true;
        }
    }

    /// Writes all dirty frames back to disk.
    ///
    /// Call this before shutting down, or at a checkpoint, to ensure no
    /// modified pages are lost. Does not evict or unpin any frames.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::Io`] if any write fails. Remaining dirty
    /// frames are still flushed even after an error (best-effort).
    pub fn flush_all(&mut self, log_manager: Option<&LogManager>) -> Result<(), StorageError> {
        let mut last_err: Option<StorageError> = None;
        let mut files_written: HashSet<u32> = HashSet::new();

        for frame_id in 0..self.capacity {
            if !self.dirty_flag[frame_id] {
                continue;
            }
            let Some((file_id, page_id)) = self.frame_to_page[frame_id] else {
                continue;
            };

            if let Some(page) = &self.frames[frame_id] {
                if let Some(lm) = log_manager {
                    if page.page_lsn() > lm.get_flushed_lsn() {
                        if let Err(e) = lm.flush() {
                            last_err = Some(e);
                            continue;
                        }
                    }
                }
            }

            let write_result = {
                let page = self.frames[frame_id].as_mut().expect("frame not found");
                page.compute_checksum();
                match self.heap_files.get_mut(&file_id) {
                    Some(heap_file) => heap_file.write_page(page_id, page),
                    None => Err(StorageError::UnknownFile(file_id)),
                }
            };

            match write_result {
                Ok(_) => {
                    self.dirty_flag[frame_id] = false;
                    files_written.insert(file_id);
                }
                Err(e) => last_err = Some(e),
            }
        }

        if last_err.is_none() {
            for file_id in files_written {
                if let Some(heap_file) = self.heap_files.get_mut(&file_id) {
                    heap_file.sync()?;
                }
            }
        }

        match last_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Returns the number of frames currently holding a cached page.
    pub fn cached_page_count(&self) -> usize {
        self.frames.iter().filter(|f| f.is_some()).count()
    }

    // ─────────────────────────────────────────────────────────────────
    // Internal helpers
    // ─────────────────────────────────────────────────────────────────

    /// Finds a free frame or evicts an existing one to make room.
    ///
    /// Priority:
    /// 1. An empty frame (`frames[i].is_none()`) — no eviction needed.
    /// 2. The unpinned frame with the smallest `last_used` (LRU victim).
    ///
    /// Returns [`StorageError::BufferPoolFull`] if all frames are pinned.
    fn find_or_evict(&mut self) -> Result<usize, StorageError> {
        // 1. Prefer an empty frame
        if let Some(frame_id) = self.frames.iter().position(|f| f.is_none()) {
            return Ok(frame_id);
        }

        // 2. Perform Clock-Sweep algorithm across frames
        // We sweep up to 2 * capacity iterations to ensure every unpinned
        // frame has its second chance cleared before selection.
        for _ in 0..(2 * self.capacity) {
            let frame_id = self.clock_hand;
            self.clock_hand = (self.clock_hand + 1) % self.capacity;

            if self.pin_count[frame_id] == 0 {
                if self.referenced[frame_id] {
                    // Give second chance and clear bit
                    self.referenced[frame_id] = false;
                } else {
                    // Victim found!
                    self.evict(frame_id)?;
                    return Ok(frame_id); // Return the evicted frame index to caller
                }
            }
        }

        // All frames are pinned
        Err(StorageError::BufferPoolFull)
    }

    /// Evicts the page currently in `frame_id`.
    ///
    /// If the frame is dirty the page is written back to disk first.
    /// After eviction the frame is empty and removed from `page_table`.
    ///
    /// # Precondition
    ///
    /// `pin_count[frame_id]` must be 0. This is enforced by
    /// [`Self::find_or_evict`] before calling here.
    fn evict(&mut self, frame_id: usize) -> Result<(), StorageError> {
        let (file_id, page_id) = self.frame_to_page[frame_id].expect("frame not found");

        if self.dirty_flag[frame_id] {
            // Write the dirty page back to disk before discarding it.
            if let Some(page) = &mut self.frames[frame_id] {
                let heap_file = self
                    .heap_files
                    .get_mut(&file_id)
                    .expect("file not registered");

                page.compute_checksum();
                heap_file.write_page(page_id, page)?;
            }
        }

        // Remove this frame's page_id from the page table.
        self.page_table.remove(&(file_id, page_id));

        // Clear the frame.
        self.frames[frame_id] = None;
        self.dirty_flag[frame_id] = false;
        self.pin_count[frame_id] = 0;
        self.referenced[frame_id] = false;
        self.frame_to_page[frame_id] = None;

        Ok(())
    }

    /// Returns the number of pages in the backing heap file.
    pub fn num_pages(&self, file_id: u32) -> Result<u32, StorageError> {
        self.heap_files
            .get(&file_id)
            .map(|hf| hf.num_pages)
            .ok_or(StorageError::UnknownFile(file_id))
    }

    /// Safely returns mutable references to two distinct frames simultaneously.
    /// Returns an error or panics if the same frame is requested twice.
    pub fn get_two_pages_mut(
        &mut self,
        frame_a: usize,
        frame_b: usize,
    ) -> (&mut TablePage, &mut TablePage) {
        // Enforce distinct frames to protect aliasing invariants
        assert!(
            frame_a != frame_b,
            "Cannot mutably borrow the same frame twice"
        );

        // Safely obtain disjoint mutable references using pointer slicing
        let frame_ptr = self.frames.as_mut_ptr();

        // Mark dirty immediately — the caller is about to modify the pages.
        self.dirty_flag[frame_a] = true;
        self.dirty_flag[frame_b] = true;

        unsafe {
            let ref_a = (&mut *frame_ptr.add(frame_a))
                .as_mut()
                .expect("frame_a is empty — did you forget to pin the page?");
            let ref_b = (&mut *frame_ptr.add(frame_b))
                .as_mut()
                .expect("frame_b is empty — did you forget to pin the page?");
            (ref_a, ref_b)
        }
    }

    /// Registers an already-opened HeapFile under `file_id` so its pages
    /// can be pinned. Must be called once per file, before any pin_page
    /// call references that file_id.
    pub fn register_file(&mut self, file_id: u32, heap_file: HeapFile) {
        self.heap_files.insert(file_id, heap_file);
    }

    /// Re-maps all internal state that referenced `old_id` to `new_id`.
    /// Use this when a `TableHeap` (which registers its file as `0`) is
    /// later assigned its "real" registry ID via `set_file_id`.
    pub fn rename_file_id(&mut self, old_id: u32, new_id: u32) {
        if old_id == new_id {
            return;
        }
        // Move the HeapFile entry
        if let Some(hf) = self.heap_files.remove(&old_id) {
            self.heap_files.insert(new_id, hf);
        }
        // Re-key every page_table entry for old_id
        let old_keys: Vec<(u32, u32)> = self
            .page_table
            .keys()
            .filter(|(fid, _)| *fid == old_id)
            .copied()
            .collect();
        for (_, page_id) in old_keys {
            if let Some(frame) = self.page_table.remove(&(old_id, page_id)) {
                self.page_table.insert((new_id, page_id), frame);
                if let Some(slot) = self.frame_to_page[frame].as_mut() {
                    slot.0 = new_id;
                }
            }
        }
    }

    /// Removes a file from the pool — flushes its dirty pages first.
    /// Call when a table/index is dropped or closed.
    pub fn unregister_file(
        &mut self,
        file_id: u32,
        log_manager: Option<&LogManager>,
    ) -> Result<(), StorageError> {
        self.flush_file(file_id, log_manager)?;
        self.heap_files.remove(&file_id);
        Ok(())
    }

    /// Writes all dirty frames belonging to `file_id` back to disk and
    /// checkpoints that file's WAL. Frames belonging to other files are
    /// left untouched.
    ///
    /// Call this when closing a table/index, or as a periodic per-file
    /// checkpoint instead of flushing the entire shared pool.
    pub fn flush_file(
        &mut self,
        file_id: u32,
        log_manager: Option<&LogManager>,
    ) -> Result<(), StorageError> {
        let mut last_err: Option<StorageError> = None;
        let mut any_written = false;

        for frame_id in 0..self.capacity {
            // Only touch frames that (a) hold a page and (b) belong to this file.
            let matches = match self.frame_to_page[frame_id] {
                Some((fid, _)) if fid == file_id => true,
                _ => false,
            };

            if !matches || !self.dirty_flag[frame_id] {
                continue;
            }

            let (_, page_id) = self.frame_to_page[frame_id].expect("checked above");

            // Enforce the WAL rule before the real write, same as evict()/flush_all().
            if let Some(page) = &self.frames[frame_id] {
                if let Some(lm) = log_manager {
                    if page.page_lsn() > lm.get_flushed_lsn() {
                        if let Err(e) = lm.flush() {
                            last_err = Some(e);
                            continue; // don't write this page if its WAL cover isn't durable
                        }
                    }
                }
            }

            // Compute checksum, then write via the CORRECT file ( not a single shared heap_file)
            let write_result = {
                let page = self.frames[frame_id].as_mut().expect("frame not found");
                page.compute_checksum();
                match self.heap_files.get_mut(&file_id) {
                    Some(heap_file) => heap_file.write_page(page_id, page),
                    None => Err(StorageError::UnknownFile(file_id)),
                }
            };

            match write_result {
                Ok(_) => {
                    self.dirty_flag[frame_id] = false;
                    any_written = true;
                }
                Err(e) => last_err = Some(e),
            }
        }

        // Only checkpoint (truncate WAL) if nothing failed — a failed write
        // means the WAL is still the only durable record of that page.
        if last_err.is_none() && any_written {
            if let Some(heap_file) = self.heap_files.get_mut(&file_id) {
                heap_file.sync()?;
            }
        }

        match last_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

impl Drop for BufferPool {
    fn drop(&mut self) {
        // Flush all dirty pages to the heap file on disk before dropping
        let _ = self.flush_all(None);
    }
}
