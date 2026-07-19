use std::collections::HashMap;

use crate::storage::{error::StorageError, file::HeapFile, page::TablePage};

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
    heap_file: HeapFile,

    /// The frame array — each slot holds one cached page or is empty.
    frames: Vec<Option<TablePage>>,

    /// Maps `page_id → frame_index` for O(1) cache-hit detection.
    page_table: HashMap<u32, usize>,

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

    /// Monotonic access timestamp per frame — used for LRU eviction.
    ///
    /// Updated to the current `clock` value every time a frame is
    /// accessed (pin or cache hit). The frame with the smallest
    /// `last_used` among unpinned frames is chosen as the eviction victim.
    last_used: Vec<u64>,

    /// Global monotonic counter incremented on every frame access.
    ///
    /// Together with `last_used` this implements a simple LRU policy
    /// without needing an explicit queue or doubly-linked list.
    clock: u64,

    /// Maximum number of frames this pool can hold simultaneously.
    capacity: usize,
}

impl BufferPool {
    /// Creates a new `BufferPool` wrapping `heap_file` with `capacity` frames.
    ///
    /// All frames start empty. No pages are loaded from disk until the first
    /// call to [`Self::pin_page`] or [`Self::new_page`].
    pub fn new(heap_file: HeapFile, capacity: usize) -> Self {
        Self {
            heap_file,
            frames: (0..capacity).map(|_| None).collect(),
            page_table: HashMap::new(),
            pin_count: vec![0; capacity],
            dirty_flag: vec![false; capacity],
            last_used: vec![0; capacity],
            clock: 0,
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
    pub fn pin_page(&mut self, page_id: u32) -> Result<usize, StorageError> {
        // ── Cache hit ────────────────────────────────────────────────
        // The page is already in a frame — just bump its pin count and
        // update the LRU timestamp; no disk access needed.
        if let Some(&frame_id) = self.page_table.get(&page_id) {
            self.clock += 1;
            self.last_used[frame_id] = self.clock;
            self.pin_count[frame_id] += 1;
            return Ok(frame_id);
        }

        // ── Cache miss ───────────────────────────────────────────────
        // Find a frame to load the page into.
        let frame_id = self.find_or_evict()?;

        // Load the page from disk into the chosen frame.
        let page = self.heap_file.read_page(page_id)?;
        self.frames[frame_id] = Some(page);

        // Register in the page table and mark as pinned.
        self.page_table.insert(page_id, frame_id);
        self.clock += 1;
        self.last_used[frame_id] = self.clock;
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
    pub fn new_page(&mut self) -> Result<(u32, usize), StorageError> {
        // Find frame first
        let frame_id = self.find_or_evict()?;

        // Allocate the page on disk first so `num_pages` is up to date
        // before `pin_page` checks bounds.
        let page_id = self.heap_file.allocate_page()?;

        // Get page from heap file using page id
        let page = self.heap_file.read_page(page_id)?;

        self.frames[frame_id] = Some(page);
        self.page_table.insert(page_id, frame_id);
        self.clock += 1;
        self.last_used[frame_id] = self.clock;
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
    pub fn flush_all(&mut self) -> Result<(), StorageError> {
        let mut last_err: Option<StorageError> = None;

        for frame_id in 0..self.capacity {
            if self.dirty_flag[frame_id] {
                if let Some(page) = &self.frames[frame_id] {
                    let page_id = *self
                        .page_table
                        .iter()
                        .find(|&(_, &fid)| fid == frame_id)
                        .map(|(pid, _)| pid)
                        .expect("frame not found in page table");
                    match self.heap_file.write_page(page_id, page) {
                        Ok(_) => self.dirty_flag[frame_id] = false,
                        Err(e) => last_err = Some(e),
                    }
                }
            }
        }

        if last_err.is_none() {
            // Every dirty page just written is now WAL-logged + written to
            // the real file. Checkpoint clears the WAL since none of those
            // records are needed for recovery anymore.
            self.heap_file.checkpoint()?;
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
        // ── 1. Prefer an empty frame ──────────────────────────────────
        if let Some(frame_id) = self.frames.iter().position(|f| f.is_none()) {
            return Ok(frame_id);
        }

        // ── 2. Find LRU unpinned frame ────────────────────────────────
        let victim = (0..self.capacity)
            .filter(|&i| self.pin_count[i] == 0)
            .min_by_key(|&i| self.last_used[i]);

        let frame_id = victim.ok_or(StorageError::BufferPoolFull)?;

        // Evict — write back to disk if dirty, then clear the frame.
        self.evict(frame_id)?;

        Ok(frame_id)
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
        let page_id = *self
            .page_table
            .iter()
            .find(|&(_, &fid)| fid == frame_id)
            .map(|(pid, _)| pid)
            .expect("frame not found in page table");

        if self.dirty_flag[frame_id] {
            // Write the dirty page back to disk before discarding it.
            if let Some(page) = &self.frames[frame_id] {
                self.heap_file.write_page(page_id, page)?;
            }
        }

        // Remove this frame's page_id from the page table.
        self.page_table.remove(&page_id);

        // Clear the frame.
        self.frames[frame_id] = None;
        self.dirty_flag[frame_id] = false;
        self.pin_count[frame_id] = 0;

        Ok(())
    }

    /// Returns the number of pages in the backing heap file.
    pub fn num_pages(&self) -> u32 {
        self.heap_file.num_pages
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
}

impl Drop for BufferPool {
    fn drop(&mut self) {
        // Flush all dirty pages to the heap file on disk before dropping
        let _ = self.flush_all();
    }
}
