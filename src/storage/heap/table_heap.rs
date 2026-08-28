use std::{
    sync::{Arc, Mutex},
    vec,
};

use crate::{
    ast::Value,
    catalog::objects::ColumnEntry,
    common::interner::Interner,
    storage::{
        BufferPool, HeapFile, Storage, StorageError,
        log::{
            log_manager::LogManager,
            log_record::{LogRecord, RecordType},
        },
        page::table_page::PageFlags,
        tuple::{
            record_id::RecordId,
            tuple::{deserialize_tuple, serialize_tuple_with_toast},
        },
        txn::transaction::Transaction,
    },
};

const DEFAULT_POOL_CAPACITY: usize = 16;

pub struct TableHeap {
    buffer_pool: Arc<Mutex<BufferPool>>,
    toast_file: Option<HeapFile>, // Lazily opened on demand
    log_manager: Option<Arc<LogManager>>,
    file_id: u32,
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
        let mut pool = BufferPool::new(DEFAULT_POOL_CAPACITY);
        pool.register_file(0, heap_file);
        let buffer_pool = Arc::new(Mutex::new(pool));
        Ok(Self {
            buffer_pool,
            toast_file: None,
            log_manager: None,
            file_id: 0,
        })
    }

    pub fn from_buffer_pool(bp: Arc<Mutex<BufferPool>>) -> Self {
        Self {
            buffer_pool: bp,
            toast_file: None,
            log_manager: None,
            file_id: 0,
        }
    }

    /// Opens or lazily initializes the toast file for this table heap.
    pub fn get_or_open_toast_file(
        &mut self,
        storage: &Storage,
        db_name: &str,
        schema_name: &str,
        table_name: &str,
    ) -> Result<&mut HeapFile, StorageError> {
        if self.toast_file.is_none() {
            let toast_path = storage.toast_path(db_name, schema_name, table_name)?;
            let toast_file = HeapFile::open(toast_path)?;
            self.toast_file = Some(toast_file);
        }

        Ok(self.toast_file.as_mut().unwrap())
    }

    /// Open with log manager
    pub fn open_with_log_manager(
        storage: &Storage,
        db_name: &str,
        schema_name: &str,
        table_name: &str,
        log_manager: Arc<LogManager>,
    ) -> Result<Self, StorageError> {
        let path = storage.table_path(db_name, schema_name, table_name)?;
        let heap_file = HeapFile::open(path)?;
        let mut pool =
            BufferPool::with_log_manager(DEFAULT_POOL_CAPACITY, Arc::clone(&log_manager));
        pool.register_file(0, heap_file);
        let buffer_pool = Arc::new(Mutex::new(pool));

        Ok(Self {
            buffer_pool,
            toast_file: None,
            log_manager: Some(log_manager),
            file_id: 0,
        })
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
        mut txn: Option<&mut Transaction>,
    ) -> Result<(u32, u16), StorageError> {
        let (bytes, has_toast) =
            serialize_tuple_with_toast(schema, values, interner, self.toast_file.as_mut())?;

        let mut bp = self
            .buffer_pool
            .lock()
            .unwrap_or_else(|err| err.into_inner());

        let (page_id, frame_id) = if bp.num_pages(self.file_id)? == 0 {
            bp.new_page(self.file_id)?
        } else {
            let last_page_id = bp.num_pages(self.file_id)? - 1;
            let frame_id = bp.pin_page(self.file_id, last_page_id)?;
            (last_page_id, frame_id)
        };

        // Trying to insert into current page
        let inserted = {
            let page = bp.get_page_mut(frame_id);
            if has_toast {
                let flags = page.flags() | PageFlags::HAS_TOAST;
                page.set_flags(flags);
            }
            page.insert_tuple(&bytes)
        };

        if let Some(slot_id) = inserted {
            // WAL Logging: if log_manager is enabled log the insert and update page_lsn
            if let Some(lm) = &self.log_manager {
                // Pulling txn_id and prev_lsn from the transaction or suing defaults
                let (txt_id, prev_lsn) = match txn.as_ref().map(|t| (t.txn_id, t.last_lsn)) {
                    Some(pair) => pair,
                    None => (0, 0),
                };
                let mut record = LogRecord {
                    lsn: 0,
                    prev_lsn,
                    txt_id,
                    record_type: RecordType::Insert,
                    file_id: self.file_id,
                    page_id,
                    offset: slot_id,
                    length: bytes.len() as u16,
                    before_image: vec![],
                    after_image: bytes.clone(),
                };
                let lsn = lm.append_record(&mut record)?;
                bp.get_page_mut(frame_id).set_page_lsn(lsn.0);

                // Updating the transaction's backward chain pointer
                if let Some(t) = txn.as_mut() {
                    t.last_lsn = lsn.0
                }
            }

            bp.unpin_page(frame_id, true);
            return Ok((page_id, slot_id));
        }

        // Current page was full allocate new page
        bp.unpin_page(frame_id, false);

        let (new_page_id, new_frame_id) = bp.new_page(self.file_id)?;

        let inserted = {
            let page = bp.get_page_mut(new_frame_id);
            if has_toast {
                let flags = page.flags() | PageFlags::HAS_TOAST;
                page.set_flags(flags);
            }
            page.insert_tuple(&bytes)
        };

        if let Some(slot_id) = inserted {
            // WAL logging for new page
            if let Some(lm) = &self.log_manager {
                let (txt_id, prev_lsn) = match txn.as_ref().map(|t| (t.txn_id, t.last_lsn)) {
                    Some(pair) => pair,
                    None => (0, 0),
                };
                let mut record = LogRecord {
                    lsn: 0,
                    prev_lsn,
                    txt_id,
                    record_type: RecordType::Insert,
                    file_id: self.file_id,
                    page_id: new_page_id,
                    offset: slot_id,
                    length: bytes.len() as u16,
                    before_image: vec![],
                    after_image: bytes,
                };
                let lsn = lm.append_record(&mut record)?;
                bp.get_page_mut(new_frame_id).set_page_lsn(lsn.0);

                // Update the transaction's backward chain pointer
                if let Some(t) = txn.as_mut() {
                    t.last_lsn = lsn.0;
                }
            }

            bp.unpin_page(new_frame_id, true);
            Ok((new_page_id, slot_id))
        } else {
            bp.unpin_page(new_frame_id, false);
            Err(StorageError::TupleError(
                "tuple too large to fit in an empty page".to_string(),
            ))
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

        for page_id in 0..bp.num_pages(self.file_id)? {
            let frame_id = bp.pin_page(self.file_id, page_id)?;

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
        let frame_id = bp.pin_page(self.file_id, rid.page_id)?;
        let page = bp.get_page(frame_id);
        let result = match page.get_tuple(rid.slot_id) {
            Some(bytes) => Some(deserialize_tuple(schema, bytes, interner)?),
            None => None,
        };
        bp.unpin_page(frame_id, false);
        Ok(result)
    }

    pub fn delete_tuple(
        &mut self,
        rid: RecordId,
        mut txn: Option<&mut Transaction>,
    ) -> Result<bool, StorageError> {
        let mut bp = self
            .buffer_pool
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let frame_id = bp.pin_page(self.file_id, rid.page_id)?;

        let before_image = match bp.get_page(frame_id).get_tuple(rid.slot_id) {
            Some(bytes) => bytes.to_vec(),
            None => {
                bp.unpin_page(frame_id, false); // Not dirty because nothing changed
                return Ok(false);
            }
        };

        bp.get_page_mut(frame_id).delete_tuple(rid.slot_id);

        // WAL logging for delete page
        if let Some(lm) = &self.log_manager {
            let (txt_id, prev_lsn) = match txn.as_ref().map(|t| (t.txn_id, t.last_lsn)) {
                Some(pair) => pair,
                None => (0, 0),
            };

            let mut record = LogRecord {
                lsn: 0,
                prev_lsn,
                txt_id,
                record_type: RecordType::Delete,
                file_id: self.file_id,
                page_id: rid.page_id,
                offset: rid.slot_id,
                length: before_image.len() as u16,
                before_image,
                after_image: vec![],
            };

            let lsn = lm.append_record(&mut record)?;
            bp.get_page_mut(frame_id).set_page_lsn(lsn.0);

            if let Some(t) = txn.as_mut() {
                t.last_lsn = lsn.0;
            }
        }

        bp.unpin_page(frame_id, true);
        Ok(true)
    }

    pub fn update_tuple(
        &mut self,

        schema: &[ColumnEntry],
        new_values: &[Value],
        interner: &Interner,
        rid: RecordId,
        mut txn: Option<&mut Transaction>,
    ) -> Result<bool, StorageError> {
        let (bytes, has_toast) =
            serialize_tuple_with_toast(schema, new_values, interner, self.toast_file.as_mut())?;

        let mut bp = self
            .buffer_pool
            .lock()
            .unwrap_or_else(|err| err.into_inner());

        let frame_id = bp.pin_page(self.file_id, rid.page_id)?;

        let before_image = match bp.get_page(frame_id).get_tuple(rid.slot_id) {
            Some(old_bytes) => old_bytes.to_vec(),
            None => {
                bp.unpin_page(frame_id, false);
                return Ok(false);
            }
        };

        bp.get_page_mut(frame_id).delete_tuple(rid.slot_id);
        let page = bp.get_page_mut(frame_id);
        if has_toast {
            let flags = page.flags() | PageFlags::HAS_TOAST;
            page.set_flags(flags);
        }
        page.insert_tuple(&bytes);

        if let Some(lm) = &self.log_manager {
            let (txt_id, prev_lsn) = match txn.as_ref().map(|t| (t.txn_id, t.last_lsn)) {
                Some(pair) => pair,
                None => (0, 0),
            };

            let mut record = LogRecord {
                lsn: 0,
                prev_lsn,
                txt_id,
                record_type: RecordType::Update,
                file_id: self.file_id,
                page_id: rid.page_id,
                offset: rid.slot_id,
                length: bytes.len() as u16,
                before_image: before_image,
                after_image: bytes,
            };

            let lsn = lm.append_record(&mut record)?;
            bp.get_page_mut(frame_id).set_page_lsn(lsn.0);

            if let Some(t) = txn.as_mut() {
                t.last_lsn = lsn.0;
            }
        }

        bp.unpin_page(frame_id, true);
        Ok(true)
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

        let num_pages = bp.num_pages(self.file_id)?;
        let mut total_reclaimed = 0;

        for page_id in 0..num_pages {
            let frame_id = bp.pin_page(self.file_id, page_id)?;

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

    pub fn set_file_id(&mut self, file_id: u32) {
        let old = self.file_id;
        self.file_id = file_id;
        self.buffer_pool
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .rename_file_id(old, file_id);
    }

    pub fn file_id(&self) -> u32 {
        self.file_id
    }
}
