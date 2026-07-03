use std::sync::{Arc, Mutex};

use crate::storage::{
    BufferPool, HeapFile, Storage, StorageError, index_page::IndexPage, record_id::RecordId,
};

pub struct BPlusTreeIndex {
    buffer_pool: Arc<Mutex<BufferPool>>,
    root_page_id: Option<u32>,
    is_unique_constraint: bool,
}

const META_PAGE_ID: u32 = 0;

impl BPlusTreeIndex {
    pub fn open(
        storage: &Storage,
        db: &str,
        schema: &str,
        index_name: &str,
        is_unique: bool,
        buffer_pool: Arc<Mutex<BufferPool>>,
    ) -> Result<Self, StorageError> {
        let _ = (storage, db, schema, index_name); // path resolution now happens where pool is opened

        let root_page_id = {
            let mut bp = buffer_pool.lock().unwrap();
            if bp.num_pages() == 0 {
                let (_, frame_id) = bp.new_page()?; // page 0
                let raw = bp.get_page_mut(frame_id);
                raw.as_bytes_mut()[0..4].copy_from_slice(&u32::MAX.to_le_bytes());
                bp.unpin_page(frame_id, true);
                None
            } else {
                let frame_id = bp.pin_page(META_PAGE_ID)?;
                let raw = bp.get_page(frame_id);
                let val = u32::from_le_bytes(raw.as_bytes()[0..4].try_into().unwrap());
                bp.unpin_page(frame_id, false);
                if val == u32::MAX { None } else { Some(val) }
            }
        };

        Ok(Self {
            buffer_pool,
            root_page_id,
            is_unique_constraint: is_unique,
        })
    }

    /// Opens a standalone index (owns its own buffer pool). Use `open` when
    /// sharing a pool with a table's other indexes.
    pub fn open_standalone(
        storage: &Storage,
        db: &str,
        schema: &str,
        index_name: &str,
        is_unique: bool,
    ) -> Result<Self, StorageError> {
        let path = storage
            .schema_path(db, schema)
            .join(format!("{}.idx", index_name));
        let heap_file = HeapFile::open(path)?;
        let buffer_pool = Arc::new(Mutex::new(BufferPool::new(heap_file, 16)));
        Self::open(storage, db, schema, index_name, is_unique, buffer_pool)
    }

    fn persist_root(&mut self) -> Result<(), StorageError> {
        let mut bp = self.buffer_pool.lock().unwrap();
        let frame_id = bp.pin_page(META_PAGE_ID)?;
        let raw = bp.get_page_mut(frame_id);
        let val = self.root_page_id.unwrap_or(u32::MAX);
        raw.as_bytes_mut()[0..4].copy_from_slice(&val.to_le_bytes());
        bp.unpin_page(frame_id, true);
        Ok(())
    }

    pub fn lookup(&mut self, key: &[u8]) -> Result<Option<RecordId>, StorageError> {
        let mut current_page_id = match self.root_page_id {
            Some(id) => id,
            None => return Ok(None),
        };

        loop {
            let mut bp = self.buffer_pool.lock().unwrap();
            let frame_id = bp.pin_page(current_page_id)?;

            let raw_page = bp.get_page(frame_id);
            let index_page = IndexPage::from_page_ref(raw_page);

            match index_page.binary_search_key(key, |raw_key_slice| raw_key_slice) {
                Ok(slot) => {
                    if index_page.is_leaf() {
                        let val_bytes = index_page.get_value(slot).unwrap();
                        let record_id = RecordId::from_bytes(val_bytes);
                        bp.unpin_page(frame_id, false);
                        return Ok(Some(record_id));
                    } else {
                        let val_bytes = index_page.get_value(slot).unwrap();
                        current_page_id = u32::from_le_bytes(val_bytes.try_into().unwrap());
                        bp.unpin_page(frame_id, false);
                    }
                }
                Err(slot) => {
                    if index_page.is_leaf() {
                        bp.unpin_page(frame_id, false);
                        return Ok(None);
                    } else {
                        if slot == 0 {
                            current_page_id = index_page.next_page_id();
                        } else {
                            let val_bytes = index_page.get_value(slot - 1).unwrap();
                            current_page_id = u32::from_le_bytes(val_bytes.try_into().unwrap())
                        }
                    }
                    bp.unpin_page(frame_id, false);
                }
            }
        }
    }

    pub fn insert(&mut self, key: &[u8], record_id: RecordId) -> Result<(), StorageError> {
        if self.is_unique_constraint {
            if self.lookup(key)?.is_some() {
                return Err(StorageError::DuplicateKey);
            }
        }
        let val_bytes = record_id.to_bytes();

        if self.root_page_id.is_none() {
            let (new_page_id, frame_id) = {
                let mut bp = self.buffer_pool.lock().unwrap();
                let (new_page_id, frame_id) = bp.new_page()?;
                let raw_page = bp.get_page_mut(frame_id);
                let index_page = IndexPage::from_page_mut(raw_page);
                index_page.set_is_leaf(true);
                index_page.set_next_page_id(0);
                index_page.insert_at(0, key, &val_bytes);
                (new_page_id, frame_id)
            };

            self.root_page_id = Some(new_page_id);
            self.persist_root()?;
            self.buffer_pool.lock().unwrap().unpin_page(frame_id, true);
            return Ok(());
        }

        let root_id = self.root_page_id.unwrap();

        if let Some((promoted_key, right_child_page_id)) =
            self.insert_recursive(root_id, key, &val_bytes)?
        {
            let (new_root_id, frame_id) = {
                let mut bp = self.buffer_pool.lock().unwrap();
                let (new_root_id, frame_id) = bp.new_page()?;
                let raw_page = bp.get_page_mut(frame_id);
                let new_root = IndexPage::from_page_mut(raw_page);
                new_root.set_is_leaf(false);
                new_root.set_next_page_id(root_id);
                new_root.insert_at(0, &promoted_key, &right_child_page_id.to_le_bytes());
                (new_root_id, frame_id)
            };

            self.root_page_id = Some(new_root_id);
            self.persist_root()?;
            self.buffer_pool.lock().unwrap().unpin_page(frame_id, true);
        }
        Ok(())
    }

    pub fn delete(&mut self, key: &[u8]) -> Result<bool, StorageError> {
        let mut current = match self.root_page_id {
            Some(id) => id,
            None => return Ok(false),
        };
        loop {
            let mut bp = self.buffer_pool.lock().unwrap();
            let frame_id = bp.pin_page(current)?;
            let is_leaf = IndexPage::from_page_ref(bp.get_page(frame_id)).is_leaf();
            if is_leaf {
                let page = IndexPage::from_page_mut(bp.get_page_mut(frame_id));
                let found = match page.binary_search_key(key, |b| b) {
                    Ok(slot) => {
                        page.remove_at(slot);
                        true
                    }
                    Err(_) => false,
                };
                bp.unpin_page(frame_id, found);
                return Ok(found);
            } else {
                let page = IndexPage::from_page_ref(bp.get_page(frame_id));
                let next = match page.binary_search_key(key, |b| b) {
                    Ok(idx) => u32::from_le_bytes(page.get_value(idx).unwrap().try_into().unwrap()),
                    Err(idx) if idx == 0 => page.next_page_id(),
                    Err(idx) => {
                        u32::from_le_bytes(page.get_value(idx - 1).unwrap().try_into().unwrap())
                    }
                };
                bp.unpin_page(frame_id, false);
                current = next;
            }
        }
    }

    fn insert_recursive(
        &mut self,
        current_page_id: u32,
        key: &[u8],
        value: &[u8],
    ) -> Result<Option<(Vec<u8>, u32)>, StorageError> {
        let mut bp = self.buffer_pool.lock().unwrap();
        let frame_id = bp.pin_page(current_page_id)?;

        let is_leaf = {
            let index_page = IndexPage::from_page_ref(bp.get_page(frame_id));
            index_page.is_leaf()
        };

        if is_leaf {
            let raw_page = bp.get_page_mut(frame_id);
            let index_page = IndexPage::from_page_mut(raw_page);

            let slot_idx = match index_page.binary_search_key(key, |b| b) {
                Ok(idx) => idx,
                Err(idx) => idx,
            };

            if index_page.insert_at(slot_idx, key, value) {
                bp.unpin_page(frame_id, true);
                return Ok(None);
            }

            let (right_page_id, right_frame_id) = bp.new_page()?;
            let (raw_left, raw_right) = unsafe {
                let left_ptr = bp.get_page_mut(frame_id) as *mut _;
                let right_ptr = bp.get_page_mut(right_frame_id) as *mut _;
                (&mut *left_ptr, &mut *right_ptr)
            };

            let left_page = IndexPage::from_page_mut(raw_left);
            let right_page = IndexPage::from_page_mut(raw_right);

            right_page.set_is_leaf(true);
            right_page.set_next_page_id(left_page.next_page_id());

            let promoted_key = left_page.split_into(right_page);
            left_page.set_next_page_id(right_page_id);

            if slot_idx < left_page.key_count() {
                left_page.insert_at(slot_idx, key, value);
            } else {
                let r_slot = slot_idx - left_page.key_count();
                right_page.insert_at(r_slot, key, value);
            }

            bp.unpin_page(frame_id, true);
            bp.unpin_page(right_frame_id, true);

            return Ok(Some((promoted_key, right_page_id)));
        } else {
            let child_page_id = {
                let index_page = IndexPage::from_page_ref(bp.get_page(frame_id));
                match index_page.binary_search_key(key, |b| b) {
                    Ok(idx) => {
                        u32::from_le_bytes(index_page.get_value(idx).unwrap().try_into().unwrap())
                    }
                    Err(idx) => {
                        if idx == 0 {
                            index_page.next_page_id()
                        } else {
                            u32::from_le_bytes(
                                index_page.get_value(idx - 1).unwrap().try_into().unwrap(),
                            )
                        }
                    }
                }
            };

            bp.unpin_page(frame_id, false);
            drop(bp); // release lock before recursing

            if let Some((promoted_key, right_child_id)) =
                self.insert_recursive(child_page_id, key, value)?
            {
                let mut bp = self.buffer_pool.lock().unwrap();
                let frame_id = bp.pin_page(current_page_id)?;
                let raw_page = bp.get_page_mut(frame_id);
                let index_page = IndexPage::from_page_mut(raw_page);

                let target_slot = match index_page.binary_search_key(&promoted_key[..], |b| b) {
                    Ok(idx) => idx,
                    Err(idx) => idx,
                };

                let right_bytes = right_child_id.to_le_bytes();
                if index_page.insert_at(target_slot, &promoted_key, &right_bytes) {
                    bp.unpin_page(frame_id, true);
                    return Ok(None);
                }

                let (right_internal_id, right_frame_id) = bp.new_page()?;
                let (raw_left, raw_right) = unsafe {
                    let left_ptr = bp.get_page_mut(frame_id) as *mut _;
                    let right_ptr = bp.get_page_mut(right_frame_id) as *mut _;
                    (&mut *left_ptr, &mut *right_ptr)
                };

                let left_internal = IndexPage::from_page_mut(raw_left);
                let right_internal = IndexPage::from_page_mut(raw_right);

                right_internal.set_is_leaf(false);

                let parent_promoted_key = left_internal.split_into(right_internal);

                if target_slot < left_internal.key_count() {
                    left_internal.insert_at(target_slot, &promoted_key, &right_bytes);
                } else if target_slot == left_internal.key_count() {
                    left_internal.insert_at(target_slot, &promoted_key, &right_bytes);
                } else {
                    let r_slot = target_slot - left_internal.key_count() - 1;
                    right_internal.insert_at(r_slot, &promoted_key, &right_bytes);
                }

                bp.unpin_page(frame_id, true);
                bp.unpin_page(right_frame_id, true);

                return Ok(Some((parent_promoted_key, right_internal_id)));
            }
        }

        Ok(None)
    }
}
