use crate::{
    ast::Value,
    catalog::objects::ColumnEntry,
    common::interner::Interner,
    storage::{
        BufferPool, HeapFile, Storage, StorageError,
        tuple::{deserialize_tuple, serialize_tuple},
    },
};

/// Default number of frames each table's buffer pool gets.
const DEFAULT_POOL_CAPACITY: usize = 16;

/// Table-level access to a heap file: serializes rows and places them into
/// pages, backed by a [`BufferPool`] so repeated access doesn't always hit
/// disk.
///
/// This is the layer the executor talks to for `INSERT`/scan operations —
/// it never touches `Page`, `BufferPool`, or `HeapFile` directly.
pub struct TableHeap {
    buffer_pool: BufferPool,
}

impl TableHeap {
    /// Opens the heap file for `(db_name, schema_name, table_name)` and
    /// wraps it in a buffer pool, ready for inserts and scans.
    pub fn open(
        storage: &Storage,
        db_name: &str,
        schema_name: &str,
        table_name: &str,
    ) -> Result<Self, StorageError> {
        let path = storage.table_path(db_name, schema_name, table_name);
        let heap_file = HeapFile::open(path)?;
        let buffer_pool = BufferPool::new(heap_file, DEFAULT_POOL_CAPACITY);
        Ok(Self { buffer_pool })
    }

    /// Serializes `values` against `schema` and inserts the resulting
    /// tuple into this table's heap file.
    ///
    /// # Page selection
    ///
    /// Always tries the last existing page first. If the file has no
    /// pages yet, or the last page doesn't have enough free space, a new
    /// page is allocated and the tuple is inserted there instead.
    ///
    /// This is a simple "always append" strategy — it does not search
    /// earlier pages for reclaimed space from deleted tuples. A free-space
    /// map (tracking the most free page per table) would be needed to do
    /// better, and is not implemented yet.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::TupleError`] if `values` does not match
    /// `schema` (wrong count, wrong type, NULL in a NOT NULL column).
    /// Returns other [`StorageError`] variants if the underlying page or
    /// disk operations fail.
    pub fn insert_tuple(
        &mut self,
        schema: &[ColumnEntry],
        values: &[Value],
        interner: &Interner,
    ) -> Result<(), StorageError> {
        // Turn the row into bytes up front — if this fails (type
        // mismatch, NOT NULL violation, etc.) we haven't touched any
        // page yet, so there's nothing to undo.
        let bytes = serialize_tuple(schema, values, interner)?;

        // Decide which page to try first: the last one if any exist,
        // otherwise allocate the table's very first page.
        let (page_id, frame_id) = if self.buffer_pool.num_pages() == 0 {
            self.buffer_pool.new_page()?
        } else {
            let last_page_id = self.buffer_pool.num_pages() - 1;
            let frame_id = self.buffer_pool.pin_page(last_page_id)?;
            (last_page_id, frame_id)
        };

        // Try inserting into that page.
        let inserted = self.buffer_pool.get_page_mut(frame_id).insert_tuple(&bytes);

        if inserted.is_some() {
            // Fit on the existing/last page — done.
            self.buffer_pool.unpin_page(frame_id, true);
            return Ok(());
        }

        // Didn't fit — release this frame without marking it dirty (we
        // didn't actually change its contents) and allocate a fresh page.
        self.buffer_pool.unpin_page(frame_id, false);

        let (_new_page_id, new_frame_id) = self.buffer_pool.new_page()?;
        let inserted = self
            .buffer_pool
            .get_page_mut(new_frame_id)
            .insert_tuple(&bytes);

        self.buffer_pool.unpin_page(new_frame_id, true);

        // A brand-new empty page should always have room for one tuple
        // unless the tuple itself is larger than a page can ever hold —
        // that case is already caught inside Page::insert_tuple (returns
        // None for tuples over u16::MAX bytes), so surface it as an error
        // rather than silently dropping the row.
        match inserted {
            Some(_) => Ok(()),
            None => Err(StorageError::TupleError(
                "tuple too large to fit in an empty page".to_string(),
            )),
        }
    }

    pub fn scan(
        &mut self,
        schema: &[ColumnEntry],
        interner: &mut Interner,
    ) -> Result<Vec<Vec<Value>>, StorageError> {
        let mut all_rows = Vec::new();

        for page_id in 0..self.buffer_pool.num_pages() {
            let frame_id = self.buffer_pool.pin_page(page_id)?;

            let page = self.buffer_pool.get_page(frame_id);

            for slot_id in 0..page.slot_count() {
                if let Some(bytes) = page.get_tuple(slot_id) {
                    let values = deserialize_tuple(schema, bytes, interner)?;
                    all_rows.push(values);
                }
            }

            self.buffer_pool.unpin_page(frame_id, false);
        }

        Ok(all_rows)
    }
}
