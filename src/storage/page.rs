use std::convert::TryInto;

/// Size of a single page in bytes.
///
/// 8KB matches PostgreSQL's default `BLCKSZ`. Every page on disk and in
/// the buffer pool is exactly this size — fixed-size pages are what make
/// `page_id * PAGE_SIZE` a valid file offset (Stage 2: `HeapFile`).
pub const PAGE_SIZE: usize = 8192;

/// Size of the page header in bytes.
///
/// Layout (little-endian):
/// ```text
/// offset 0..4   page_id            (u32)
/// offset 4..6   slot_count         (u16)
/// offset 6..8   free_space_pointer (u16)
/// ```
/// 8 bytes total, naturally aligned.
const HEADER_SIZE: usize = 8;

/// Size of a single slot array entry in bytes.
///
/// Layout (little-endian):
/// ```text
/// offset 0..2   tuple offset (u16) — byte offset into `data` where the tuple starts
/// offset 2..4   tuple length (u16) — length in bytes; 0 means "deleted/empty slot"
/// ```
const SLOT_SIZE: usize = 4;

/// A fixed-size, disk-block-sized page using a slotted layout.
///
/// # Layout
///
/// ```text
/// ┌─────────────────────────────────────────────────┐  byte 0
/// │ Header (8 bytes): page_id, slot_count, fsp       │
/// ├─────────────────────────────────────────────────┤  byte HEADER_SIZE
/// │ Slot array — grows DOWNWARD as slots are added   │
/// │   slot 0 │ slot 1 │ slot 2 │ ...                  │
/// ├─────────────────────────────────────────────────┤  ← free space
/// │              (free space)                        │
/// ├─────────────────────────────────────────────────┤  ← free_space_pointer
/// │ Tuple data — grows UPWARD (toward lower offsets) │
/// │   ... │ tuple 2 │ tuple 1 │ tuple 0               │
/// └─────────────────────────────────────────────────┘  byte PAGE_SIZE
/// ```
///
/// - The slot array starts right after the header and grows toward the
///   end of the page as more tuples are inserted.
/// - Tuple data is appended starting from the *end* of the page and grows
///   toward the start. `free_space_pointer` always points at the start of
///   the most recently written tuple — i.e. the boundary between free
///   space and tuple data.
/// - A slot's `(offset, length)` pair tells you exactly where its tuple's
///   bytes live in `data`. This indirection is what lets `delete_tuple`
///   remove a tuple without shifting every other tuple's bytes — only the
///   slot is marked empty.
///
/// # Why slotted pages?
///
/// Tuples are variable-length (a `VARCHAR(255)` row is rarely 255 bytes).
/// A slotted page lets each tuple be exactly as long as it needs to be,
/// while the fixed-size slot array gives every tuple a stable numeric
/// identifier (`slot_id`) that other parts of the engine (indexes, the
/// buffer pool, MVCC version chains) can reference even if the tuple's
/// bytes later move during compaction.
pub struct Page {
    data: [u8; PAGE_SIZE],
}

impl Page {
    /// Creates a new, empty page with the given `page_id`.
    ///
    /// Initializes the header so that:
    /// - `slot_count = 0` — no tuples yet
    /// - `free_space_pointer = PAGE_SIZE` — free space extends all the way
    ///   to the end of the page (no tuple data written yet)
    pub fn new(page_id: u32) -> Self {
        let mut page = Self {
            data: [0u8; PAGE_SIZE],
        };

        page.set_page_id(page_id);
        page.set_slot_count(0);
        // Free space pointer starts at PAGE_SIZE: the first tuple written
        // will be placed at `PAGE_SIZE - tuple.len()`, growing the "used"
        // region from the end of the page backward.
        page.set_free_space_pointer(PAGE_SIZE as u16);

        page
    }

    // ─────────────────────────────────────────────────────────────────
    // Header accessors
    // ─────────────────────────────────────────────────────────────────

    /// Returns this page's identifier.
    pub fn page_id(&self) -> u32 {
        u32::from_le_bytes(self.data[0..4].try_into().unwrap())
    }

    fn set_page_id(&mut self, page_id: u32) {
        self.data[0..4].copy_from_slice(&page_id.to_le_bytes());
    }

    /// Returns the number of slots in the slot array.
    ///
    /// This includes deleted/empty slots — `slot_count` only ever grows
    /// (Stage 1 does not reclaim/reuse slot indices). A slot with length 0
    /// is "deleted" but its index remains allocated.
    pub fn slot_count(&self) -> u16 {
        u16::from_le_bytes(self.data[4..6].try_into().unwrap())
    }

    fn set_slot_count(&mut self, count: u16) {
        self.data[4..6].copy_from_slice(&count.to_le_bytes());
    }

    /// Returns the current free-space pointer.
    ///
    /// This is the byte offset of the start of the most-recently-written
    /// tuple — equivalently, the boundary between free space (below this
    /// offset... no, *above*, toward the slot array) and tuple data
    /// (at and above this offset, toward the end of the page).
    ///
    /// Free space is the region `[slot_array_end, free_space_pointer)`.
    fn free_space_pointer(&self) -> u16 {
        u16::from_le_bytes(self.data[6..8].try_into().unwrap())
    }

    fn set_free_space_pointer(&mut self, ptr: u16) {
        self.data[6..8].copy_from_slice(&ptr.to_le_bytes());
    }

    /// Returns the number of bytes currently available for a new tuple
    /// *and* its slot entry.
    ///
    /// ```text
    /// slot_array_end = HEADER_SIZE + slot_count * SLOT_SIZE
    /// free_space     = free_space_pointer - slot_array_end
    /// ```
    ///
    /// This is the space between the end of the slot array and the start
    /// of the tuple data region. Both regions grow toward each other as
    /// tuples are inserted, so this value shrinks with every insertion
    /// (by `tuple.len() + SLOT_SIZE` for a brand-new slot, or just
    /// `tuple.len()` if an existing empty slot is reused — see
    /// [`Self::insert_tuple`]).
    pub fn free_space(&self) -> usize {
        let slot_array_end = HEADER_SIZE + self.slot_count() as usize * SLOT_SIZE;
        self.free_space_pointer() as usize - slot_array_end
    }

    // ─────────────────────────────────────────────────────────────────
    // Slot array accessors
    // ─────────────────────────────────────────────────────────────────

    /// Returns the byte offset where slot `slot_id`'s entry begins.
    fn slot_entry_offset(slot_id: u16) -> usize {
        HEADER_SIZE + slot_id as usize * SLOT_SIZE
    }

    /// Reads slot `slot_id`'s `(tuple_offset, tuple_length)` pair.
    ///
    /// Returns `None` if `slot_id` is out of range (`>= slot_count`).
    /// A returned `(_, 0)` means the slot exists but its tuple was deleted.
    fn read_slot(&self, slot_id: u16) -> Option<(u16, u16)> {
        if slot_id >= self.slot_count() {
            return None;
        }

        let off = Self::slot_entry_offset(slot_id);
        let tuple_offset = u16::from_le_bytes(self.data[off..off + 2].try_into().unwrap());
        let tuple_length = u16::from_le_bytes(self.data[off + 2..off + 4].try_into().unwrap());
        Some((tuple_offset, tuple_length))
    }

    /// Writes slot `slot_id`'s `(tuple_offset, tuple_length)` pair.
    ///
    /// Does not check `slot_id` against `slot_count` — callers must ensure
    /// the slot has already been accounted for (either via
    /// `set_slot_count` for a new slot, or because the slot already
    /// existed).
    fn write_slot(&mut self, slot_id: u16, tuple_offset: u16, tuple_length: u16) {
        let off = Self::slot_entry_offset(slot_id);
        self.data[off..off + 2].copy_from_slice(&tuple_offset.to_le_bytes());
        self.data[off + 2..off + 4].copy_from_slice(&tuple_length.to_le_bytes());
    }

    // ─────────────────────────────────────────────────────────────────
    // Tuple operations
    // ─────────────────────────────────────────────────────────────────

    /// Inserts `tuple` into this page and returns its slot id.
    ///
    /// Returns `None` if there is not enough free space.
    ///
    /// # Slot reuse
    ///
    /// Before allocating a brand-new slot, this scans existing slots for
    /// one marked deleted (`length == 0`) and reuses it if found — this
    /// avoids unbounded `slot_count` growth when tuples are repeatedly
    /// deleted and re-inserted. A reused slot only needs space for the new
    /// tuple's bytes (no extra `SLOT_SIZE` — the slot entry already exists).
    /// A brand-new slot needs `tuple.len() + SLOT_SIZE` of free space,
    /// since both the tuple bytes *and* a new slot entry must fit.
    ///
    /// # Panics
    ///
    /// Does not panic. `tuple.len()` must fit in a `u16` (max 65535 bytes);
    /// callers inserting larger tuples (e.g. `TEXT` columns) will need
    /// out-of-line/overflow storage in a later stage — not handled here.
    pub fn insert_tuple(&mut self, tuple: &[u8]) -> Option<u16> {
        let tuple_len = tuple.len();
        // A single tuple must fit in a u16 length field.
        if tuple_len > u16::MAX as usize {
            return None;
        }

        // Look for a deleted slot to reuse.
        let reusable_slot =
            (0..self.slot_count()).find(|&id| matches!(self.read_slot(id), Some((_, 0))));

        let needed = if reusable_slot.is_some() {
            tuple_len
        } else {
            tuple_len + SLOT_SIZE
        };

        let slot_array_end = HEADER_SIZE + self.slot_count() as usize * SLOT_SIZE;
        let fsp = self.free_space_pointer() as usize;
        if fsp < slot_array_end || needed > fsp - slot_array_end {
            return None;
        }

        // New tuple data is written just below the current free-space
        // pointer, growing the tuple-data region toward the slot array.
        let new_fsp = self.free_space_pointer() as usize - tuple_len;
        self.data[new_fsp..new_fsp + tuple_len].copy_from_slice(tuple);
        self.set_free_space_pointer(new_fsp as u16);

        let slot_id = match reusable_slot {
            Some(id) => id,
            None => {
                let id = self.slot_count();
                self.set_slot_count(id + 1);
                id
            }
        };

        self.write_slot(slot_id, new_fsp as u16, tuple_len as u16);
        Some(slot_id)
    }

    /// Returns the bytes for the tuple stored in `slot_id`.
    ///
    /// Returns `None` if `slot_id` is out of range or the slot has been
    /// deleted (length 0).
    pub fn get_tuple(&self, slot_id: u16) -> Option<&[u8]> {
        let (offset, length) = self.read_slot(slot_id)?;
        if length == 0 {
            return None;
        }
        let offset = offset as usize;
        let length = length as usize;
        Some(&self.data[offset..offset + length])
    }

    /// Marks the tuple in `slot_id` as deleted.
    ///
    /// Returns `true` if the slot existed and was not already deleted,
    /// `false` if `slot_id` is out of range or already deleted.
    ///
    /// # Note on space reclamation
    ///
    /// This is a "lazy delete" — the tuple's bytes remain in `data` and
    /// are not reclaimed; only the slot's length is zeroed, which makes
    /// [`Self::get_tuple`] return `None` and allows [`Self::insert_tuple`]
    /// to reuse the slot *index* (but not necessarily the freed bytes,
    /// which become unreachable fragmentation). Compacting the page to
    /// reclaim fragmented space is a later optimization (page compaction /
    /// vacuum), not implemented here.
    pub fn delete_tuple(&mut self, slot_id: u16) -> bool {
        match self.read_slot(slot_id) {
            Some((_, 0)) | None => false,
            Some((offset, _)) => {
                self.write_slot(slot_id, offset, 0);
                true
            }
        }
    }

    /// Constructs a page from a raw on-disk page image.
    ///
    /// `HeapFile::read_page` reads exactly `PAGE_SIZE` bytes from disk and
    /// passes them here to reconstruct the in-memory `Page` representation.
    ///
    /// No validation is performed — the caller is responsible for ensuring
    /// the bytes originated from a valid page written by this storage engine.
    pub fn from_bytes(data: [u8; PAGE_SIZE]) -> Self {
        Self { data }
    }

    /// Returns the page's raw byte representation.
    ///
    /// The returned slice contains the complete page image, including the
    /// header, slot array, free space, and tuple data regions. This is used
    /// by `HeapFile::write_page` to persist the page to disk with a single
    /// `write_all` call.
    ///
    /// The returned bytes should be treated as opaque by callers; page
    /// contents should normally be accessed through the page API rather than
    /// by manually parsing the byte array.
    pub fn as_bytes(&self) -> &[u8; PAGE_SIZE] {
        &self.data
    }

    /// Returns the mutable pages's raw byte
    pub fn as_bytes_mut(&mut self) -> &mut [u8; PAGE_SIZE] {
        &mut self.data
    }
}
