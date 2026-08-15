use std::sync::{Arc, Mutex};

use crate::storage::{
    BufferPool, HeapFile, Storage, StorageError, page::IndexPage, tuple::record_id::RecordId,
};

/// Page 0 of every index file — never used as a tree node.
/// Layout: [0..4] root_page_id, [4..8] free_list_head. NIL (u32::MAX) means empty.
const META_PAGE_ID: u32 = 0;

/// Minimum keys a non-root node must hold before triggering merge/redistribute.
const MIN_KEYS: u16 = 2;

const NIL: u32 = u32::MAX;

pub struct BPlusTreeIndex {
    buffer_pool: Arc<Mutex<BufferPool>>,
    root_page_id: Option<u32>,
    /// Head of the on-disk free page list. Freed pages are linked via
    /// `IndexPage::write_next_free` and reused by `alloc_page` before
    /// ever extending the file.
    free_head: u32,
    is_unique_constraint: bool,
}

impl BPlusTreeIndex {
    /// Opens an index against a shared buffer pool. Reads root + free-list
    /// head from the meta page (page 0), creating it if the file is new.
    pub fn open(
        is_unique: bool,
        buffer_pool: Arc<Mutex<BufferPool>>,
    ) -> Result<Self, StorageError> {
        let (root_page_id, free_head) = {
            let mut bp = buffer_pool.lock().unwrap_or_else(|err| err.into_inner());
            if bp.num_pages() == 0 {
                let (_, frame_id) = bp.new_page()?; // page 0
                let raw = bp.get_page_mut(frame_id);
                raw.as_bytes_mut()[0..4].copy_from_slice(&NIL.to_le_bytes());
                raw.as_bytes_mut()[4..8].copy_from_slice(&NIL.to_le_bytes());
                bp.unpin_page(frame_id, true);
                (None, NIL)
            } else {
                let frame_id = bp.pin_page(META_PAGE_ID)?;
                let raw = bp.get_page(frame_id);
                let root = u32::from_le_bytes(raw.as_bytes()[0..4].try_into().unwrap());
                let free = u32::from_le_bytes(raw.as_bytes()[4..8].try_into().unwrap());
                bp.unpin_page(frame_id, false);
                (if root == NIL { None } else { Some(root) }, free)
            }
        };

        Ok(Self {
            buffer_pool,
            root_page_id,
            free_head,
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
        if !storage.schema_dir_exists(db, schema) {
            return Err(StorageError::DirectoryNotFound(
                storage.schema_path(db, schema),
            ));
        }
        let path = storage
            .schema_path(db, schema)
            .join(format!("{}.idx", index_name));
        let heap_file = HeapFile::open(path)?;
        let buffer_pool = Arc::new(Mutex::new(BufferPool::new(heap_file, 16)));
        Self::open(is_unique, buffer_pool)
    }

    /// Writes root + free-list head to the meta page. Call after either
    /// changes. Locks the pool itself — never call while already holding
    /// a lock (use inline writes instead, as `alloc_page_locked` does).
    fn persist_meta(&mut self) -> Result<(), StorageError> {
        let mut bp = self
            .buffer_pool
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let frame_id = bp.pin_page(META_PAGE_ID)?;
        let raw = bp.get_page_mut(frame_id);
        let root = self.root_page_id.unwrap_or(NIL);
        raw.as_bytes_mut()[0..4].copy_from_slice(&root.to_le_bytes());
        raw.as_bytes_mut()[4..8].copy_from_slice(&self.free_head.to_le_bytes());
        bp.unpin_page(frame_id, true);
        Ok(())
    }

    pub fn lookup(&mut self, key: &[u8]) -> Result<Option<RecordId>, StorageError> {
        let mut current_page_id = match self.root_page_id {
            Some(id) => id,
            None => return Ok(None),
        };

        loop {
            let mut bp = self
                .buffer_pool
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            let frame_id = bp.pin_page(current_page_id)?;

            let raw_page = bp.get_page(frame_id);
            let index_page = IndexPage::from_page_ref(raw_page);

            match index_page.binary_search_key(key, |raw_key_slice| raw_key_slice) {
                Ok(slot) => {
                    if index_page.is_leaf() {
                        let val_bytes = index_page.get_value(slot).unwrap();
                        let record_id = RecordId::from_bytes(val_bytes)?;
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
        if self.is_unique_constraint && self.lookup(key)?.is_some() {
            return Err(StorageError::DuplicateKey);
        }
        let val_bytes = record_id.to_bytes();

        // ── Case 1: empty index — allocate first leaf as root ──
        if self.root_page_id.is_none() {
            let (new_page_id, frame_id) = self.alloc_page()?;
            {
                let mut bp = self
                    .buffer_pool
                    .lock()
                    .unwrap_or_else(|err| err.into_inner());
                let raw_page = bp.get_page_mut(frame_id);
                let mut index_page = IndexPage::from_page_mut(raw_page);
                index_page.init(true, 0);
                index_page.insert_at(0, key, &val_bytes);
                bp.unpin_page(frame_id, true);
            }
            self.root_page_id = Some(new_page_id);
            self.persist_meta()?;
            return Ok(());
        }

        // ── Case 2: normal traversal, root split propagates up here ──
        let root_id = self.root_page_id.unwrap();

        if let Some((promoted_key, right_child_page_id)) =
            self.insert_recursive(root_id, key, &val_bytes)?
        {
            let (new_root_id, frame_id) = self.alloc_page()?;
            {
                let mut bp = self
                    .buffer_pool
                    .lock()
                    .unwrap_or_else(|err| err.into_inner());
                let raw_page = bp.get_page_mut(frame_id);
                let mut new_root = IndexPage::from_page_mut(raw_page);
                new_root.init(false, root_id);
                new_root.insert_at(0, &promoted_key, &right_child_page_id.to_le_bytes());
                bp.unpin_page(frame_id, true);
            }
            self.root_page_id = Some(new_root_id);
            self.persist_meta()?;
        }
        Ok(())
    }

    /// Descends to the target leaf, inserting and propagating splits upward.
    /// Returns `Some((promoted_key, right_child_page_id))` on a child split.
    fn insert_recursive(
        &mut self,
        current_page_id: u32,
        key: &[u8],
        value: &[u8],
    ) -> Result<Option<(Vec<u8>, u32)>, StorageError> {
        let mut bp = self
            .buffer_pool
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let frame_id = bp.pin_page(current_page_id)?;

        let is_leaf = IndexPage::from_page_ref(bp.get_page(frame_id)).is_leaf();

        if is_leaf {
            let mut index_page = IndexPage::from_page_mut(bp.get_page_mut(frame_id));

            let slot_idx = match index_page.binary_search_key(key, |b| b) {
                Ok(idx) | Err(idx) => idx,
            };

            if index_page.insert_at(slot_idx, key, value) {
                bp.unpin_page(frame_id, true);
                return Ok(None);
            }

            // Full — split. `bp` is already locked here, so grab the new
            // page via the locked helper (self.alloc_page() would deadlock).
            let (right_page_id, right_frame_id) =
                Self::alloc_page_locked(&mut self.free_head, &mut bp)?;
            let (raw_left, raw_right) = bp.get_two_pages_mut(frame_id, right_frame_id);

            let mut left_page = IndexPage::from_page_mut(raw_left);
            let mut right_page = IndexPage::from_page_mut(raw_right);

            right_page.init(true, left_page.next_page_id());

            let promoted_key = left_page.split_into(&mut right_page);
            left_page.set_next_page_id(right_page_id);

            if slot_idx < left_page.key_count() {
                left_page.insert_at(slot_idx, key, value);
            } else {
                let r_slot = slot_idx - left_page.key_count();
                right_page.insert_at(r_slot, key, value);
            }

            bp.unpin_page(frame_id, true);
            bp.unpin_page(right_frame_id, true);
            drop(bp);
            self.persist_meta()?; // free_head may have moved

            return Ok(Some((promoted_key, right_page_id)));
        }

        // ── Internal routing ──
        let child_page_id = {
            let index_page = IndexPage::from_page_ref(bp.get_page(frame_id));
            match index_page.binary_search_key(key, |b| b) {
                Ok(idx) => {
                    u32::from_le_bytes(index_page.get_value(idx).unwrap().try_into().unwrap())
                }
                Err(idx) if idx == 0 => index_page.next_page_id(),
                Err(idx) => {
                    u32::from_le_bytes(index_page.get_value(idx - 1).unwrap().try_into().unwrap())
                }
            }
        };

        bp.unpin_page(frame_id, false);
        drop(bp); // release lock before recursing

        if let Some((promoted_key, right_child_id)) =
            self.insert_recursive(child_page_id, key, value)?
        {
            let mut bp = self
                .buffer_pool
                .lock()
                .unwrap_or_else(|err| err.into_inner());
            let frame_id = bp.pin_page(current_page_id)?;
            let mut index_page = IndexPage::from_page_mut(bp.get_page_mut(frame_id));

            let target_slot = match index_page.binary_search_key(&promoted_key[..], |b| b) {
                Ok(idx) | Err(idx) => idx,
            };

            let right_bytes = right_child_id.to_le_bytes();
            if index_page.insert_at(target_slot, &promoted_key, &right_bytes) {
                bp.unpin_page(frame_id, true);
                return Ok(None);
            }

            // Internal node full — split it too.
            let (right_internal_id, right_frame_id) =
                Self::alloc_page_locked(&mut self.free_head, &mut bp)?;
            let (raw_left, raw_right) = unsafe {
                let left_ptr = bp.get_page_mut(frame_id) as *mut _;
                let right_ptr = bp.get_page_mut(right_frame_id) as *mut _;
                (&mut *left_ptr, &mut *right_ptr)
            };

            let mut left_internal = IndexPage::from_page_mut(raw_left);
            let mut right_internal = IndexPage::from_page_mut(raw_right);

            right_internal.init(false, 0);

            let parent_promoted_key = left_internal.split_into(&mut right_internal);

            if target_slot <= left_internal.key_count() {
                left_internal.insert_at(target_slot, &promoted_key, &right_bytes);
            } else {
                let r_slot = target_slot - left_internal.key_count() - 1;
                right_internal.insert_at(r_slot, &promoted_key, &right_bytes);
            }

            bp.unpin_page(frame_id, true);
            bp.unpin_page(right_frame_id, true);
            drop(bp);
            self.persist_meta()?;

            return Ok(Some((parent_promoted_key, right_internal_id)));
        }

        Ok(None)
    }

    /// Public delete entry point. Handles root collapse after the
    /// recursive delete leaves the root as an empty internal node.
    pub fn delete_public(&mut self, key: &[u8]) -> Result<bool, StorageError> {
        let result = self.delete(key)?;
        if let Some(root) = self.root_page_id {
            let (collapse, new_root) = {
                let mut bp = self
                    .buffer_pool
                    .lock()
                    .unwrap_or_else(|err| err.into_inner());
                let frame_id = bp.pin_page(root)?;
                let page = IndexPage::from_page_ref(bp.get_page(frame_id));
                let collapse = !page.is_leaf() && page.key_count() == 0;
                let new_root = if collapse {
                    Some(page.next_page_id())
                } else {
                    None
                };
                bp.unpin_page(frame_id, false);
                (collapse, new_root)
            };
            if collapse {
                let old_root = root;
                self.root_page_id = new_root;
                self.persist_meta()?;
                self.free_page(old_root)?;
            }
        }
        Ok(result)
    }

    fn delete(&mut self, key: &[u8]) -> Result<bool, StorageError> {
        let root = match self.root_page_id {
            Some(id) => id,
            None => return Ok(false),
        };
        let (found, _) = self.delete_recursive(root, key)?;
        Ok(found)
    }

    /// Returns `(found, underflow)` — `underflow` tells the caller whether
    /// this node dropped below `MIN_KEYS` and needs merge/redistribute.
    fn delete_recursive(&mut self, page_id: u32, key: &[u8]) -> Result<(bool, bool), StorageError> {
        let mut bp = self
            .buffer_pool
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let frame_id = bp.pin_page(page_id)?;
        let is_leaf = IndexPage::from_page_ref(bp.get_page(frame_id)).is_leaf();

        if is_leaf {
            let mut page = IndexPage::from_page_mut(bp.get_page_mut(frame_id));
            let found = match page.binary_search_key(key, |b| b) {
                Ok(slot) => {
                    page.remove_at(slot);
                    true
                }
                Err(_) => false,
            };
            let underflow = page.key_count() < MIN_KEYS;
            bp.unpin_page(frame_id, found);
            return Ok((found, underflow));
        }

        let child_id = {
            let page = IndexPage::from_page_ref(bp.get_page(frame_id));
            match page.binary_search_key(key, |b| b) {
                Ok(idx) => u32::from_le_bytes(page.get_value(idx).unwrap().try_into().unwrap()),
                Err(idx) if idx == 0 => page.next_page_id(),
                Err(idx) => {
                    u32::from_le_bytes(page.get_value(idx - 1).unwrap().try_into().unwrap())
                }
            }
        };
        bp.unpin_page(frame_id, false);
        drop(bp);

        let (found, child_underflow) = self.delete_recursive(child_id, key)?;

        if child_underflow {
            self.fix_underflow(page_id, child_id)?;
        }

        let mut bp = self
            .buffer_pool
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let frame_id = bp.pin_page(page_id)?;
        let underflow = IndexPage::from_page_ref(bp.get_page(frame_id)).key_count() < MIN_KEYS;
        bp.unpin_page(frame_id, false);

        Ok((found, underflow))
    }

    /// Rebalances `child_id` under `parent_id`: borrow from a sibling with
    /// spare keys, else merge with one and free the emptied page.
    fn fix_underflow(&mut self, parent_id: u32, child_id: u32) -> Result<(), StorageError> {
        let mut bp = self
            .buffer_pool
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        let parent_frame = bp.pin_page(parent_id)?;

        let (left_sib, right_sib, child_slot) = {
            let parent = IndexPage::from_page_ref(bp.get_page(parent_frame));
            let mut left = None;
            let mut right = None;
            let mut slot_of_child = None;

            if parent.next_page_id() == child_id {
                slot_of_child = Some(0u16);
                if parent.key_count() > 0 {
                    right = Some(u32::from_le_bytes(
                        parent.get_value(0).unwrap().try_into().unwrap(),
                    ));
                }
            } else {
                for i in 0..parent.key_count() {
                    let cid = u32::from_le_bytes(parent.get_value(i).unwrap().try_into().unwrap());
                    if cid == child_id {
                        slot_of_child = Some(i + 1);
                        left = Some(if i == 0 {
                            parent.next_page_id()
                        } else {
                            u32::from_le_bytes(parent.get_value(i - 1).unwrap().try_into().unwrap())
                        });
                        if i + 1 < parent.key_count() {
                            right = Some(u32::from_le_bytes(
                                parent.get_value(i + 1).unwrap().try_into().unwrap(),
                            ));
                        }
                        break;
                    }
                }
            }
            (left, right, slot_of_child)
        };
        bp.unpin_page(parent_frame, false);

        // Try borrowing from the right sibling first.
        if let Some(right_id) = right_sib {
            let child_frame = bp.pin_page(child_id)?;
            let right_frame = bp.pin_page(right_id)?;
            let right_has_extra =
                IndexPage::from_page_ref(bp.get_page(right_frame)).key_count() > MIN_KEYS;

            if right_has_extra {
                let (raw_c, raw_r) = unsafe {
                    let c = bp.get_page_mut(child_frame) as *mut _;
                    let r = bp.get_page_mut(right_frame) as *mut _;
                    (&mut *c, &mut *r)
                };
                let mut child = IndexPage::from_page_mut(raw_c);
                let mut rsib = IndexPage::from_page_mut(raw_r);

                let k = rsib.get_key(0).unwrap().to_vec();
                let v = rsib.get_value(0).unwrap().to_vec();
                child.insert_at(child.key_count(), &k, &v);
                rsib.remove_at(0);

                bp.unpin_page(child_frame, true);
                bp.unpin_page(right_frame, true);
                return Ok(());
            }

            // No spare keys — merge child + right into child, free right.
            let (raw_c, raw_r) = unsafe {
                let c = bp.get_page_mut(child_frame) as *mut _;
                let r = bp.get_page_mut(right_frame) as *mut _;
                (&mut *c, &mut *r)
            };
            let mut child = IndexPage::from_page_mut(raw_c);
            let rsib = IndexPage::from_page_mut(raw_r);
            for i in 0..rsib.key_count() {
                let k = rsib.get_key(i).unwrap().to_vec();
                let v = rsib.get_value(i).unwrap().to_vec();
                child.insert_at(child.key_count(), &k, &v);
            }
            if child.is_leaf() {
                child.set_next_page_id(rsib.next_page_id());
            }
            bp.unpin_page(child_frame, true);
            bp.unpin_page(right_frame, false);

            let parent_frame = bp.pin_page(parent_id)?;
            let mut parent = IndexPage::from_page_mut(bp.get_page_mut(parent_frame));
            if let Some(slot) = child_slot {
                parent.remove_at(slot);
            }
            bp.unpin_page(parent_frame, true);

            Self::free_page_locked(&mut self.free_head, &mut bp, right_id);
            drop(bp);
            self.persist_meta()?;
            return Ok(());
        }

        // No right sibling — try the left one.
        if let Some(left_id) = left_sib {
            let child_frame = bp.pin_page(child_id)?;
            let left_frame = bp.pin_page(left_id)?;
            let left_has_extra =
                IndexPage::from_page_ref(bp.get_page(left_frame)).key_count() > MIN_KEYS;

            if left_has_extra {
                let (raw_c, raw_l) = unsafe {
                    let c = bp.get_page_mut(child_frame) as *mut _;
                    let l = bp.get_page_mut(left_frame) as *mut _;
                    (&mut *c, &mut *l)
                };
                let mut child = IndexPage::from_page_mut(raw_c);
                let mut lsib = IndexPage::from_page_mut(raw_l);

                let last = lsib.key_count() - 1;
                let k = lsib.get_key(last).unwrap().to_vec();
                let v = lsib.get_value(last).unwrap().to_vec();
                lsib.remove_at(last);
                child.insert_at(0, &k, &v);

                bp.unpin_page(child_frame, true);
                bp.unpin_page(left_frame, true);
                return Ok(());
            }

            // No spare keys — merge left + child into left, free child.
            let (raw_l, raw_c) = unsafe {
                let l = bp.get_page_mut(left_frame) as *mut _;
                let c = bp.get_page_mut(child_frame) as *mut _;
                (&mut *l, &mut *c)
            };
            let mut lsib = IndexPage::from_page_mut(raw_l);
            let child = IndexPage::from_page_mut(raw_c);
            for i in 0..child.key_count() {
                let k = child.get_key(i).unwrap().to_vec();
                let v = child.get_value(i).unwrap().to_vec();
                lsib.insert_at(lsib.key_count(), &k, &v);
            }
            if lsib.is_leaf() {
                lsib.set_next_page_id(child.next_page_id());
            }
            bp.unpin_page(left_frame, true);
            bp.unpin_page(child_frame, false);

            let parent_frame = bp.pin_page(parent_id)?;
            let mut parent = IndexPage::from_page_mut(bp.get_page_mut(parent_frame));
            if let Some(slot) = child_slot {
                let remove_slot = if slot == 0 { 0 } else { slot - 1 };
                parent.remove_at(remove_slot);
            }
            bp.unpin_page(parent_frame, true);

            Self::free_page_locked(&mut self.free_head, &mut bp, child_id);
            drop(bp);
            self.persist_meta()?;
        }

        Ok(())
    }

    // ── Free-list — unlocked (self-locking) variants for standalone use ──

    /// Grabs a page — reuses a freed one if available, else allocates fresh.
    /// Locks the pool itself; do not call while already holding a lock.
    fn alloc_page(&mut self) -> Result<(u32, usize), StorageError> {
        let mut bp = self
            .buffer_pool
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        Self::alloc_page_locked(&mut self.free_head, &mut bp)
    }

    /// Returns `page_id` to the free list. Locks the pool itself; do not
    /// call while already holding a lock.
    fn free_page(&mut self, page_id: u32) -> Result<(), StorageError> {
        let mut bp = self
            .buffer_pool
            .lock()
            .unwrap_or_else(|err| err.into_inner());
        Self::free_page_locked(&mut self.free_head, &mut bp, page_id);
        drop(bp);
        self.persist_meta()
    }

    // ── Free-list — locked variants, take an already-held guard ──
    // Use these anywhere the caller is already inside a `bp.lock()`
    // section, to avoid re-locking the (non-reentrant) mutex.

    fn alloc_page_locked(
        free_head: &mut u32,
        bp: &mut BufferPool,
    ) -> Result<(u32, usize), StorageError> {
        if *free_head != NIL {
            let page_id = *free_head;
            let frame_id = bp.pin_page(page_id)?;
            let next_free = IndexPage::from_page_ref(bp.get_page(frame_id)).read_next_free();
            *free_head = next_free;
            return Ok((page_id, frame_id));
        }
        bp.new_page()
    }

    fn free_page_locked(free_head: &mut u32, bp: &mut BufferPool, page_id: u32) {
        if let Ok(frame_id) = bp.pin_page(page_id) {
            let mut page = IndexPage::from_page_mut(bp.get_page_mut(frame_id));
            page.write_next_free(*free_head);
            bp.unpin_page(frame_id, true);
            *free_head = page_id;
        }
    }
}
