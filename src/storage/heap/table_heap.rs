use std::sync::{Arc, Mutex};

use crate::{
    ast::Value,
    catalog::objects::ColumnEntry,
    common::interner::Interner,
    storage::{
        BufferPool, HeapFile, Storage, StorageError,
        record_id::RecordId,
        tuple::{deserialize_tuple, serialize_tuple},
    },
};

const DEFAULT_POOL_CAPACITY: usize = 16;

pub struct TableHeap {
    buffer_pool: Arc<Mutex<BufferPool>>,
}

impl TableHeap {
    pub fn open(
        storage: &Storage,
        db_name: &str,
        schema_name: &str,
        table_name: &str,
    ) -> Result<Self, StorageError> {
        let path = storage.table_path(db_name, schema_name, table_name)?;
        let heap_file = HeapFile::open(path)?;
        let buffer_pool = Arc::new(Mutex::new(BufferPool::new(
            heap_file,
            DEFAULT_POOL_CAPACITY,
        )));
        Ok(Self { buffer_pool })
    }

    /// Returns a clone of this table's shared buffer pool handle, so its
    /// indexes can be opened against the same pool.
    pub fn buffer_pool_handle(&self) -> Arc<Mutex<BufferPool>> {
        Arc::clone(&self.buffer_pool)
    }

    pub fn insert_tuple(
        &mut self,
        schema: &[ColumnEntry],
        values: &[Value],
        interner: &Interner,
    ) -> Result<(u32, u16), StorageError> {
        let bytes = serialize_tuple(schema, values, interner)?;

        let mut bp = self
            .buffer_pool
            .lock()
            .unwrap_or_else(|err| err.into_inner());

        let (page_id, frame_id) = if bp.num_pages() == 0 {
            bp.new_page()?
        } else {
            let last_page_id = bp.num_pages() - 1;
            let frame_id = bp.pin_page(last_page_id)?;
            (last_page_id, frame_id)
        };

        let inserted = bp.get_page_mut(frame_id).insert_tuple(&bytes);

        if let Some(slot_id) = inserted {
            bp.unpin_page(frame_id, true);
            return Ok((page_id, slot_id));
        }

        bp.unpin_page(frame_id, false);

        let (new_page_id, new_frame_id) = bp.new_page()?;
        let inserted = bp.get_page_mut(new_frame_id).insert_tuple(&bytes);

        bp.unpin_page(new_frame_id, true);

        match inserted {
            Some(slot_id) => Ok((new_page_id, slot_id)),
            None => Err(StorageError::TupleError(
                "tuple too large to fit in an empty page".to_string(),
            )),
        }
    }

    pub fn scan(
        &mut self,
        schema: &[ColumnEntry],
        interner: &Interner,
    ) -> Result<Vec<Vec<Value>>, StorageError> {
        let mut all_rows = Vec::new();
        let mut bp = self
            .buffer_pool
            .lock()
            .unwrap_or_else(|err| err.into_inner());

        for page_id in 0..bp.num_pages() {
            let frame_id = bp.pin_page(page_id)?;

            let page = bp.get_page(frame_id);

            for slot_id in 0..page.slot_count() {
                if let Some(bytes) = page.get_tuple(slot_id) {
                    let values = deserialize_tuple(schema, bytes, interner)?;
                    all_rows.push(values);
                }
            }

            bp.unpin_page(frame_id, false);
        }

        Ok(all_rows)
    }

    pub fn from_buffer_pool(bp: Arc<Mutex<BufferPool>>) -> Self {
        Self { buffer_pool: bp }
    }

    pub fn get_tuple(
        &mut self,
        rid: RecordId,
        schema: &[ColumnEntry],
        interner: &Interner,
    ) -> Result<Option<Vec<Value>>, StorageError> {
        let mut bp = self
            .buffer_pool
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let frame_id = bp.pin_page(rid.page_id)?;
        let page = bp.get_page(frame_id);
        let result = match page.get_tuple(rid.slot_id) {
            Some(bytes) => Some(deserialize_tuple(schema, bytes, interner)?),
            None => None,
        };
        bp.unpin_page(frame_id, false);
        Ok(result)
    }

    /// Scans all pages in the table heap and compacts any pages
    /// that have fragmented space from deleted tuples.
    ///
    /// Returns the total number of bytes reclaimed across all pages.
    pub fn vacuum(&mut self) -> Result<usize, StorageError> {
        let mut bp = self
            .buffer_pool
            .lock()
            .unwrap_or_else(|err| err.into_inner());

        let num_pages = bp.num_pages();
        let mut total_reclaimed = 0;

        for page_id in 0..num_pages {
            let frame_id = bp.pin_page(page_id)?;

            let (fragmented, dirty) = {
                let page = bp.get_page(frame_id);
                let frag = page.fragmented_space();
                (frag, frag > 0)
            };

            if dirty {
                let page = bp.get_page_mut(frame_id);
                page.compact();
                total_reclaimed += fragmented;
            }

            bp.unpin_page(frame_id, dirty);
        }

        Ok(total_reclaimed)
    }
}
