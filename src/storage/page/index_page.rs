use std::cmp::Ordering;

use crate::storage::page::TablePage;

pub const PAGE_SIZE: usize = 8192;
const INDEX_HEADER_SIZE: usize = 9; // 1 (is_leaf) + 2 (key_count) + 4 (next_page_id) + 2 (fsp)
const INDEX_SLOT_SIZE: usize = 6; // 2 (payload_offset) + 2 (key_len) + 2 (value_len)

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
        page.set_free_space_pointer(PAGE_SIZE as u16);
        page
    }

    pub fn init(&mut self, is_leaf: bool, next_page_id: u32) {
        self.data.fill(0);
        self.set_is_leaf(is_leaf);
        self.set_key_count(0);
        self.set_next_page_id(next_page_id);
        self.set_free_space_pointer(PAGE_SIZE as u16);
    }

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
        u16::from_le_bytes(self.data[7..9].try_into().unwrap())
    }

    fn set_free_space_pointer(&mut self, ptr: u16) {
        self.data[7..9].copy_from_slice(&ptr.to_le_bytes());
    }

    pub fn free_space(&self) -> usize {
        let slot_array_end = INDEX_HEADER_SIZE + (self.key_count() as usize * INDEX_SLOT_SIZE);
        self.free_space_pointer() as usize - slot_array_end
    }

    pub fn insert_at(&mut self, slot_idx: u16, key: &[u8], value: &[u8]) -> bool {
        let count = self.key_count();
        if slot_idx > count {
            return false;
        }

        let payload_len = key.len() + value.len();
        let total_needed = INDEX_SLOT_SIZE + payload_len;

        if total_needed > self.free_space() {
            return false;
        }

        let target_slot_off = INDEX_HEADER_SIZE + slot_idx as usize * INDEX_SLOT_SIZE;
        let current_slot_array_end = INDEX_HEADER_SIZE + count as usize * INDEX_SLOT_SIZE;

        if (slot_idx as usize) < count as usize {
            self.data.copy_within(
                target_slot_off..current_slot_array_end,
                target_slot_off + INDEX_SLOT_SIZE,
            );
        }

        let new_fsp = self.free_space_pointer() as usize - payload_len;
        self.data[new_fsp..new_fsp + key.len()].copy_from_slice(key);
        self.data[new_fsp + key.len()..new_fsp + payload_len].copy_from_slice(value);

        self.data[target_slot_off..target_slot_off + 2]
            .copy_from_slice(&(new_fsp as u16).to_le_bytes());
        self.data[target_slot_off + 2..target_slot_off + 4]
            .copy_from_slice(&(key.len() as u16).to_le_bytes());
        self.data[target_slot_off + 4..target_slot_off + 6]
            .copy_from_slice(&(value.len() as u16).to_le_bytes());

        self.set_key_count(count + 1);
        self.set_free_space_pointer(new_fsp as u16);
        true
    }

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

    pub fn get_value(&self, slot_idx: u16) -> Option<&[u8]> {
        if slot_idx >= self.key_count() {
            return None;
        }

        let slot_off = INDEX_HEADER_SIZE + slot_idx as usize * INDEX_SLOT_SIZE;
        let payload_off =
            u16::from_le_bytes(self.data[slot_off..slot_off + 2].try_into().unwrap()) as usize;
        let key_len =
            u16::from_le_bytes(self.data[slot_off + 2..slot_off + 4].try_into().unwrap()) as usize;
        let val_len =
            u16::from_le_bytes(self.data[slot_off + 4..slot_off + 6].try_into().unwrap()) as usize;

        let value_off = payload_off + key_len;
        Some(&self.data[value_off..value_off + val_len])
    }

    pub fn split_into(&mut self, right_page: &mut IndexPage) -> Vec<u8> {
        let total_count = self.key_count();
        let mid = total_count / 2;

        let split_key = self.get_key(mid).unwrap().to_vec();

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

        self.set_key_count(mid);
        self.compact();

        split_key
    }

    pub fn remove_at(&mut self, slot_idx: u16) {
        let count = self.key_count();
        let slot_off = INDEX_HEADER_SIZE + slot_idx as usize * INDEX_SLOT_SIZE;
        let end = INDEX_HEADER_SIZE + count as usize * INDEX_SLOT_SIZE;
        self.data
            .copy_within(slot_off + INDEX_SLOT_SIZE..end, slot_off);
        self.set_key_count(count - 1);
        self.compact();
    }

    pub fn compact(&mut self) {
        let count = self.key_count() as usize;
        if count == 0 {
            self.set_key_count(0);
            self.set_free_space_pointer(PAGE_SIZE as u16);
            return;
        }

        let mut temp_buffer = [0u8; PAGE_SIZE];
        let mut temp_fsp = PAGE_SIZE;

        let mut new_offsets = vec![0u16; count];

        for i in 0..count {
            let slot_off = INDEX_HEADER_SIZE + i * INDEX_SLOT_SIZE;

            let old_payload_off =
                u16::from_le_bytes(self.data[slot_off..slot_off + 2].try_into().unwrap()) as usize;
            let key_len =
                u16::from_le_bytes(self.data[slot_off + 2..slot_off + 4].try_into().unwrap())
                    as usize;
            let val_len =
                u16::from_le_bytes(self.data[slot_off + 4..slot_off + 6].try_into().unwrap())
                    as usize;
            let payload_len = key_len + val_len;

            temp_fsp -= payload_len;

            temp_buffer[temp_fsp..temp_fsp + payload_len]
                .copy_from_slice(&self.data[old_payload_off..old_payload_off + payload_len]);

            new_offsets[i] = temp_fsp as u16;
        }

        self.data[temp_fsp..PAGE_SIZE].copy_from_slice(&temp_buffer[temp_fsp..PAGE_SIZE]);

        for i in 0..count {
            let slot_off = INDEX_HEADER_SIZE + i * INDEX_SLOT_SIZE;
            self.data[slot_off..slot_off + 2].copy_from_slice(&new_offsets[i].to_le_bytes());
        }

        self.set_free_space_pointer(temp_fsp as u16);
    }

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

    pub fn from_page_ref(page: &TablePage) -> &Self {
        unsafe { &*(page as *const TablePage as *const IndexPage) }
    }

    pub fn from_page_mut(page: &mut TablePage) -> &mut Self {
        unsafe { &mut *(page as *mut TablePage as *mut IndexPage) }
    }

    pub fn write_next_free(&mut self, next: u32) {
        self.data[0..4].copy_from_slice(&next.to_le_bytes());
    }

    pub fn read_next_free(&self) -> u32 {
        u32::from_be_bytes(self.data[0..4].try_into().unwrap())
    }
}
