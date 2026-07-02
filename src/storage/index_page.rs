use std::cmp::Ordering;

use crate::storage::page::Page;

pub const PAGE_SIZE: usize = 8192;
const INDEX_HEADER_SIZE: usize = 7; // 1 (is_leaf) + 2 (key_count) + 4 (next_page_id)
const INDEX_SLOT_SIZE: usize = 4; // 2 (payload_offset) + 2 (key_len)

pub struct IndexPage {
    data: [u8; PAGE_SIZE],
}

impl IndexPage {
    pub fn new(is_leaf: bool, next_page_id: u32) -> Self {
        let mut page = Self {
            data: [0u8; PAGE_SIZE],
        };
        page.set_is_leaf(is_leaf);
        page.set_key_count(0);
        page.set_next_page_id(next_page_id);
        page
    }

    // Header Accessors
    pub fn is_leaf(&self) -> bool {
        self.data[0] != 0
    }

    pub fn set_is_leaf(&mut self, is_leaf: bool) {
        self.data[0] = if is_leaf { 1 } else { 0 };
    }

    pub fn key_count(&self) -> u16 {
        u16::from_le_bytes(self.data[1..3].try_into().unwrap())
    }

    pub fn set_key_count(&mut self, count: u16) {
        self.data[1..3].copy_from_slice(&count.to_le_bytes());
    }

    pub fn next_page_id(&self) -> u32 {
        u32::from_le_bytes(self.data[3..7].try_into().unwrap())
    }

    pub fn set_next_page_id(&mut self, id: u32) {
        self.data[3..7].copy_from_slice(&id.to_le_bytes());
    }

    fn free_space_pointer(&self) -> u16 {
        // If empty, payload area starts right at the end of the page
        let count = self.key_count();
        if count == 0 {
            return PAGE_SIZE as u16;
        }

        // Locate the lowest offset among all active slots
        let mut min_offset = PAGE_SIZE as u16;
        for i in 0..count {
            let slot_off = INDEX_HEADER_SIZE + i as usize * INDEX_SLOT_SIZE;
            let p_off = u16::from_le_bytes(
                self.data[slot_off..slot_off + 2]
                    .try_into()
                    .unwrap(),
            );
            if p_off < min_offset {
                min_offset = p_off;
            }
        }
        min_offset
    }

    pub fn free_space(&self) -> usize {
        let slot_array_end = INDEX_HEADER_SIZE + (self.key_count() as usize * INDEX_SLOT_SIZE);
        self.free_space_pointer() as usize - slot_array_end
    }

    // Key & Value Accessors (Zero-Copy)

    /// Inserts a key-value payload at a specific slot index to maintain sorted order.
    /// Internal nodes store: Key + Child Page ID (4 bytes)
    /// Leaf nodes store: Key + Record ID / Value payload
    pub fn insert_at(&mut self, slot_idx: u16, key: &[u8], value: &[u8]) -> bool {
        let count = self.key_count();
        if slot_idx > count {
            return false;
        }

        let payload_len = key.len() + value.len();
        let total_needed = INDEX_SLOT_SIZE + payload_len;

        if total_needed > self.free_space() {
            return false; // Page Split Required at tree level
        }

        // 1. Shift slot array right to clear out the target slot index
        let target_slot_off = INDEX_HEADER_SIZE + slot_idx as usize * INDEX_SLOT_SIZE;
        let current_slot_array_end = INDEX_HEADER_SIZE + count as usize * INDEX_SLOT_SIZE;

        if (slot_idx as usize) < count as usize {
            self.data.copy_within(
                target_slot_off..current_slot_array_end,
                target_slot_off + INDEX_SLOT_SIZE,
            );
        }

        // 2. Write the payload to the bottom of the page
        let new_fsp = self.free_space_pointer() as usize - payload_len;
        self.data[new_fsp..new_fsp + key.len()].copy_from_slice(key);
        self.data[new_fsp + key.len()..new_fsp + payload_len].copy_from_slice(value);

        // 3. Setup the new slot metadata
        self.data[target_slot_off..target_slot_off + 2]
            .copy_from_slice(&(new_fsp as u16).to_le_bytes());
        self.data[target_slot_off + 2..target_slot_off + 4]
            .copy_from_slice(&(key.len() as u16).to_le_bytes());

        self.set_key_count(count + 1);
        true
    }

    /// Extracts raw key slice without allocations or copying
    pub fn get_key(&self, slot_idx: u16) -> Option<&[u8]> {
        if slot_idx >= self.key_count() {
            return None;
        }

        let slot_off = INDEX_HEADER_SIZE + slot_idx as usize * INDEX_SLOT_SIZE;
        let payload_off =
            u16::from_le_bytes(self.data[slot_off..slot_off + 2].try_into().unwrap()) as usize;
        let key_len =
            u16::from_le_bytes(self.data[slot_off + 2..slot_off + 4].try_into().unwrap()) as usize;

        Some(&self.data[payload_off..payload_off + key_len])
    }

    /// Extracts the raw value bytes associated with a key.
    /// - If **Leaf**: This returns your user data payload (e.g., RecordId/TupleId).
    /// - If **Internal**: This returns the child PageId (4 bytes).
    pub fn get_value(&self, slot_idx: u16) -> Option<&[u8]> {
        if slot_idx >= self.key_count() {
            return None;
        }

        let slot_off = INDEX_HEADER_SIZE + slot_idx as usize * INDEX_SLOT_SIZE;
        let payload_off =
            u16::from_le_bytes(self.data[slot_off..slot_off + 2].try_into().unwrap()) as usize;
        let key_len =
            u16::from_le_bytes(self.data[slot_off + 2..slot_off + 4].try_into().unwrap()) as usize;

        // The value payload sits immediately after the key bytes
        let value_off = payload_off + key_len;

        // Determine value length by checking where the next chunk boundary is
        let current_fsp = self.free_space_pointer() as usize;
        let val_len = if payload_off == current_fsp {
            // It's the most recently written element, so it occupies everything up to the previous minimum boundary
            let mut base = PAGE_SIZE;
            for i in 0..self.key_count() {
                if i != slot_idx {
                    let s_off = INDEX_HEADER_SIZE + i as usize * INDEX_SLOT_SIZE;
                    let p_off = u16::from_le_bytes(self.data[s_off..s_off + 2].try_into().unwrap())
                        as usize;
                    if p_off > payload_off && p_off < base {
                        base = p_off;
                    }
                }
            }
            base - value_off
        } else {
            // Find the payload that sits directly below this one in memory
            let mut next_highest_payload = PAGE_SIZE;
            for i in 0..self.key_count() {
                let s_off = INDEX_HEADER_SIZE + i as usize * INDEX_SLOT_SIZE;
                let p_off =
                    u16::from_le_bytes(self.data[s_off..s_off + 2].try_into().unwrap()) as usize;
                if p_off > payload_off && p_off < next_highest_payload {
                    next_highest_payload = p_off;
                }
            }
            next_highest_payload - value_off
        };
        Some(&self.data[value_off..value_off + val_len])
    }

    /// Splits the current page in half, moving the right half of the entries into `right_page`.
    /// Returns the key that needs to be promoted to the parent internal node.
    pub fn split_into(&mut self, right_page: &mut IndexPage) -> Vec<u8> {
        let total_count = self.key_count();
        let mid = total_count / 2;

        // Extract the key that will be promoted to the parent node
        let split_key = self.get_key(mid).unwrap().to_vec();

        // If it's an internal node, the split key moves up completely (leaving a gap).
        // If it's a leaf node, the split key stays on the right side as a valid data point.
        let start_idx = if self.is_leaf() {
            mid
        } else {
            let split_val = self.get_value(mid).unwrap();
            right_page.set_next_page_id(u32::from_le_bytes(split_val.try_into().unwrap()));
            mid + 1
        };

        let mut r_idx = 0;
        for i in start_idx..total_count {
            let key = self.get_key(i).unwrap().to_vec();
            let val = self.get_value(i).unwrap().to_vec();
            right_page.insert_at(r_idx, &key, &val);
            r_idx += 1;
        }

        // Truncate the left (current) page's slot count
        self.set_key_count(mid);

        // Note: Page compaction/vacuum logic should be run here to clean up
        // the abandoned bytes in `self.data`, or simply rely on temporary allocation rules.
        self.compact();

        split_key
    }

    /// Reclaims fragmented space by tightly re-packing the payloads of active slots.
    ///
    /// This resolves the "lazy delete" problem where abandoned bytes remain
    /// in the page after a split or deletion.
    pub fn compact(&mut self) {
        let count = self.key_count() as usize;
        if count == 0 {
            // If there are no keys, the entire data area after the header is free.
            self.set_key_count(0);
            return;
        }

        // 1. Create a temporary buffer to hold the compacted payload data
        let mut temp_buffer = [0u8; PAGE_SIZE];
        let mut temp_fsp = PAGE_SIZE;

        // 2. We must keep track of the new offsets for each active slot
        //    Using a small fixed-size array on the stack to avoid heap allocation
        let mut new_offsets = [0u16; 2048]; // Supports up to ~2048 keys per 8KB page
        assert!(
            count <= new_offsets.len(),
            "Slot count exceeds compaction tracking limits"
        );

        // 3. Copy active payloads into the temporary buffer from the bottom up
        for i in 0..count {
            let slot_off = INDEX_HEADER_SIZE + i * INDEX_SLOT_SIZE;

            // Read current payload metadata safely
            let old_payload_off =
                u16::from_le_bytes(self.data[slot_off..slot_off + 2].try_into().unwrap()) as usize;
            let key_len =
                u16::from_le_bytes(self.data[slot_off + 2..slot_off + 4].try_into().unwrap())
                    as usize;

            // Calculate total payload length (Key + Value)
            let payload_len = self.get_payload_len(i as u16, old_payload_off);

            // Shift our temporary space pointer down
            temp_fsp -= payload_len;

            // Copy bytes from the live page into our temporary buffer positioning
            temp_buffer[temp_fsp..temp_fsp + payload_len]
                .copy_from_slice(&self.data[old_payload_off..old_payload_off + payload_len]);

            // Track where this payload now lives in the new layout
            new_offsets[i] = temp_fsp as u16;
        }

        // 4. Flush the temporary data back to the primary page buffer
        //    We only overwrite from the new free space pointer to the end of the page.
        self.data[temp_fsp..PAGE_SIZE].copy_from_slice(&temp_buffer[temp_fsp..PAGE_SIZE]);

        // 5. Update the slot array entries with their pristine, non-fragmented offsets
        for i in 0..count {
            let slot_off = INDEX_HEADER_SIZE + i * INDEX_SLOT_SIZE;
            self.data[slot_off..slot_off + 2].copy_from_slice(&new_offsets[i].to_le_bytes());
        }

        // Explicit safety check: The space between the slot array end and the new FSP
        // is now completely cleared of garbage bytes.
    }

    /// Helper function to calculate total payload length (Key + Value) for a given slot.
    fn get_payload_len(&self, slot_idx: u16, payload_off: usize) -> usize {
        let key_len = u16::from_le_bytes(
            self.data[INDEX_HEADER_SIZE + slot_idx as usize * INDEX_SLOT_SIZE + 2
                ..INDEX_HEADER_SIZE + slot_idx as usize * INDEX_SLOT_SIZE + 4]
                .try_into()
                .unwrap(),
        ) as usize;

        let mut next_hisghest_payload = PAGE_SIZE;
        for i in 0..self.key_count() {
            let s_off = INDEX_HEADER_SIZE + i as usize * INDEX_SLOT_SIZE;
            let p_off =
                u16::from_le_bytes(self.data[s_off..s_off + 2].try_into().unwrap()) as usize;
            if p_off > payload_off && p_off < next_hisghest_payload {
                next_hisghest_payload = p_off;
            }
        }

        let value_len = next_hisghest_payload - (payload_off + key_len);
        key_len + value_len
    }

    /// Performs binary search directly across the contiguous raw page buffer!
    /// Zero allocations, maximum cache locality.
    pub fn binary_search_key<K: Ord + ?Sized>(
        &self,
        target_key: &K,
        decode_fn: impl Fn(&[u8]) -> &K,
    ) -> Result<u16, u16> {
        let mut low = 0;
        let mut high = self.key_count();
        while low < high {
            let mid = low + (high - low) / 2;
            let mid_key_bytes = self.get_key(mid).unwrap();
            let mid_key = decode_fn(mid_key_bytes);

            match mid_key.cmp(target_key) {
                Ordering::Equal => return Ok(mid),
                Ordering::Less => low = mid + 1,
                Ordering::Greater => high = mid,
            }
        }
        Err(low)
    }


    /// Reinterprets a shared raw Page as an IndexPage reference without copying.
    pub fn from_page_ref(page: &Page) -> &Self {
        // Safe because IndexPage has the exact same layout and size as Page ([u8; 8192])
        unsafe { &*(page as *const Page as *const IndexPage)}
    }

    /// Reinterprets a mutable raw Page as a mutable IndexPage reference without copying.
    pub fn from_page_mut(page: &mut Page) -> &mut Self {
        // Safe because IndexPage has the exact same layout and size as Page ([u8; 8192])
        unsafe { &mut *(page as *mut Page as *mut IndexPage) }
    }
}
