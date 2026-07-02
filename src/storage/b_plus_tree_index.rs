use crate::storage::{
    BufferPool, HeapFile, Storage, StorageError, index_page::IndexPage, record_id::RecordId,
};

pub struct BPlusTreeIndex {
    pub buffer_pool: BufferPool,
    root_page_id: Option<u32>,
    is_unique_constraint: bool,
}

impl BPlusTreeIndex {
    pub fn open(
        storage: &Storage,
        db: &str,
        schema: &str,
        index_name: &str,
        is_unique: bool,
    ) -> Result<Self, StorageError> {
        // Generate index file path under the schema directory (e.g., public/users_pkey.idx)
        let path = storage
            .schema_path(db, schema)
            .join(format!("{}.idx", index_name));
        let heap_file = HeapFile::open(path)?;
        let buffer_pool = BufferPool::new(heap_file, 16); // capacity matches to DEFAULT_POOL_CAPACITY

        Ok(Self {
            buffer_pool,
            root_page_id: None,
            is_unique_constraint: is_unique,
        })
    }

    /// Primary Key, Unique Key, and Secondary Index Retrieval Pass
    pub fn lookup(&mut self, key: &[u8]) -> Result<Option<RecordId>, StorageError> {
        let mut current_page_id = match self.root_page_id {
            Some(id) => id,
            None => return Ok(None), // Index is completely empty
        };

        loop {
            // Pin the page into the buffer pool frame
            let frame_id = self.buffer_pool.pin_page(current_page_id)?;

            // Cast the regular page into our optimized IndexPage reference
            let raw_page = self.buffer_pool.get_page(frame_id);
            let index_page = IndexPage::from_page_ref(raw_page);

            // Execute binary search directly over raw page bytes
            match index_page.binary_search_key(key, |raw_key_slice| raw_key_slice) {
                Ok(slot) => {
                    if index_page.is_leaf() {
                        let val_bytes = index_page.get_value(slot).unwrap();
                        let record_id = RecordId::from_bytes(val_bytes);
                        self.buffer_pool.unpin_page(frame_id, false);
                        return Ok(Some(record_id));
                    } else {
                        // Internal routing match
                        let val_bytes = index_page.get_value(slot).unwrap();
                        current_page_id = u32::from_le_bytes(val_bytes.try_into().unwrap());
                        self.buffer_pool.unpin_page(frame_id, false);
                    }
                }
                Err(slot) => {
                    if index_page.is_leaf() {
                        self.buffer_pool.unpin_page(frame_id, false);
                        return Ok(None); // Key doesn't exist
                    } else {
                        // Follow appropriate branch down internal tree node
                        if slot == 0 {
                            current_page_id = index_page.next_page_id();
                        } else {
                            let val_bytes = index_page.get_value(slot - 1).unwrap();
                            current_page_id = u32::from_le_bytes(val_bytes.try_into().unwrap())
                        }
                    }
                    self.buffer_pool.unpin_page(frame_id, false);
                }
            }
        }
    }

    /// Inserts a new Key -> RecordId mapping into the disk-backed B+Tree.
    pub fn insert(&mut self, key: &[u8], record_id: RecordId) -> Result<(), StorageError> {
        let val_bytes = record_id.to_bytes();

        // ── Case 1: The Index is Completely Empty ──
        if self.root_page_id.is_none() {
            // Allocate a brand-new page from the buffer pool
            let (new_page_id, frame_id) = self.buffer_pool.new_page()?;

            // Get mutable access to initialized raw page bytes
            let raw_page = self.buffer_pool.get_page_mut(frame_id);
            let index_page = IndexPage::from_page_mut(raw_page);

            // Reconfigure this fresh root as a leaf node with 0 as the next link
            index_page.set_is_leaf(true);
            index_page.set_next_page_id(0);

            // Insert our very first element
            index_page.insert_at(0, key, &val_bytes);

            // Save state, mark the buffer dirty, and unpin
            self.root_page_id = Some(new_page_id);
            self.buffer_pool.unpin_page(frame_id, true);
            return Ok(());
        }

        // ── Case 2: Standard B+Tree Traversal & Insertion ──
        let root_id = self.root_page_id.unwrap();

        if let Some((promoted_key, right_child_page_id)) =
            self.insert_recursive(root_id, key, &val_bytes)?
        {
            // Root split occurred, must construct a brand-new internal Root node
            let (new_root_id, frame_id) = self.buffer_pool.new_page()?;
            let raw_page = self.buffer_pool.get_page_mut(frame_id);
            let new_root = IndexPage::from_page_mut(raw_page);

            new_root.set_is_leaf(false);
            // Rule: The leftmost child pointer (child 0) goes into next_page_id
            new_root.set_next_page_id(root_id);

            // Insert the split/promoted boundary key pointing to our newly allocated right subtree
            new_root.insert_at(0, &promoted_key, &right_child_page_id.to_le_bytes());

            self.root_page_id = Some(new_root_id);
            self.buffer_pool.unpin_page(frame_id, true);
        }
        Ok(())
    }

    /// Recursively descends the B+Tree nodes to perform inserts and propagates splits upwards.
    /// Returns `Some((promoted_key, right_child_page_id))` if a child split takes place.
    fn insert_recursive(
        &mut self,
        current_page_id: u32,
        key: &[u8],
        value: &[u8],
    ) -> Result<Option<(Vec<u8>, u32)>, StorageError> {
        let frame_id = self.buffer_pool.pin_page(current_page_id)?;

        // Check if the current node page layout is a leaf or internal router
        let is_leaf = {
            let index_page = IndexPage::from_page_ref(self.buffer_pool.get_page(frame_id));
            index_page.is_leaf()
        };

        if is_leaf {
            let raw_page = self.buffer_pool.get_page_mut(frame_id);
            let index_page = IndexPage::from_page_mut(raw_page);

            let slot_idx = match index_page.binary_search_key(key, |b| b) {
                Ok(idx) => idx, // Duplicate handling choice
                Err(idx) => idx,
            };

            // Attempt to pack our payload directly onto the page
            if index_page.insert_at(slot_idx, key, value) {
                self.buffer_pool.unpin_page(frame_id, true); // true = dirty[cite: 1]
                return Ok(None);
            }

            // Node is full! Allocate a sibling IndexPage to shift half our elements into[cite: 1, 4]
            let (right_page_id, right_frame_id) = self.buffer_pool.new_page()?;
            let (raw_left, raw_right) = unsafe {
                // Perform quick unsafe double-pointer extraction to safely split
                // between two concurrently pinned buffer frames
                let left_ptr = self.buffer_pool.get_page_mut(frame_id) as *mut _;
                let right_ptr = self.buffer_pool.get_page_mut(right_frame_id) as *mut _;
                (&mut *left_ptr, &mut *right_ptr)
            };

            let left_page = IndexPage::from_page_mut(raw_left);
            let right_page = IndexPage::from_page_mut(raw_right);

            right_page.set_is_leaf(true);
            right_page.set_next_page_id(left_page.next_page_id());

            // Move the right half of elements out and isolate the promoted median key boundary[cite: 4]
            let promoted_key = left_page.split_into(right_page);
            left_page.set_next_page_id(right_page_id);

            // Re-evaluate which side of our new page split the incoming payload should drop into
            if slot_idx < left_page.key_count() {
                left_page.insert_at(slot_idx, key, value);
            } else {
                let r_slot = slot_idx - left_page.key_count();
                right_page.insert_at(r_slot, key, value);
            }

            self.buffer_pool.unpin_page(frame_id, true); //[cite: 1]
            self.buffer_pool.unpin_page(right_frame_id, true); //[cite: 1]

            return Ok(Some((promoted_key, right_page_id)));
        } else {
            // ── Internal Node Routing Path ──
            let child_page_id = {
                let index_page = IndexPage::from_page_ref(self.buffer_pool.get_page(frame_id));
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

            // Release our lookahead frame pin so deeper recursion levels don't exhaust pool capacity
            self.buffer_pool.unpin_page(frame_id, false);

            // Recurse downwards to locate leaf level destination paths
            if let Some((promoted_key, right_child_id)) =
                self.insert_recursive(child_page_id, key, value)?
            {
                // A lower child split propagated up! Re-pin our current routing block to update map guides[cite: 1]
                let frame_id = self.buffer_pool.pin_page(current_page_id)?;
                let raw_page = self.buffer_pool.get_page_mut(frame_id);
                let index_page = IndexPage::from_page_mut(raw_page);

                let target_slot = match index_page.binary_search_key(&promoted_key[..], |b| b) {
                    Ok(idx) => idx,
                    Err(idx) => idx,
                };

                let right_bytes = right_child_id.to_le_bytes();
                if index_page.insert_at(target_slot, &promoted_key, &right_bytes) {
                    self.buffer_pool.unpin_page(frame_id, true); //[cite: 1]
                    return Ok(None);
                }

                // Internal Node itself is full! Execute internal page splitting protocol[cite: 1, 4]
                let (right_internal_id, right_frame_id) = self.buffer_pool.new_page()?;
                let (raw_left, raw_right) = unsafe {
                    let left_ptr = self.buffer_pool.get_page_mut(frame_id) as *mut _;
                    let right_ptr = self.buffer_pool.get_page_mut(right_frame_id) as *mut _;
                    (&mut *left_ptr, &mut *right_ptr)
                };

                let left_internal = IndexPage::from_page_mut(raw_left);
                let right_internal = IndexPage::from_page_mut(raw_right);

                right_internal.set_is_leaf(false);

                // Split internal node entries[cite: 4]
                let parent_promoted_key = left_internal.split_into(right_internal);

                // Insert the new split-key that came up from below into whichever half fits it
                if target_slot < left_internal.key_count() {
                    left_internal.insert_at(target_slot, &promoted_key, &right_bytes);
                } else {
                    let r_slot = target_slot - left_internal.key_count() - 1;
                    right_internal.insert_at(r_slot, &promoted_key, &right_bytes);
                }

                self.buffer_pool.unpin_page(frame_id, true); //[cite: 1]
                self.buffer_pool.unpin_page(right_frame_id, true); //[cite: 1]

                return Ok(Some((parent_promoted_key, right_internal_id)));
            }
        }

        Ok(None)
    }
}
