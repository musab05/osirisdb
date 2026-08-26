use std::cmp::Ordering;

use crate::storage::page::TablePage;
pub use crate::storage::page::raw_page::PAGE_SIZE;

// ─────────────────────────────────────────────────────────────────────────────
// IndexPage layout
// ─────────────────────────────────────────────────────────────────────────────
//
// Every IndexPage is stored inside a TablePage frame managed by the BufferPool.
// The BufferPool writes shared header fields to the first 24 bytes on every
// flush (page_id, page_lsn, checksum, slot_count, fsp, page_type, flags).
//
// To avoid corruption the IndexPage-specific header fields are placed entirely
// AFTER byte 24, leaving bytes 0..24 exclusively owned by the BufferPool /
// TablePage layer.
//
// Byte map:
//
//   0.. 4  page_id           } TablePage / BufferPool — DO NOT TOUCH
//   4..12  page_lsn          }
//  12..16  checksum          }
//  16..18  slot_count        }
//  18..20  fsp (heap)        }
//  20..22  page_type         }
//  22..24  flags             }
//  ──────────────────────────────────────────────────
//  24      is_leaf           } IndexPage header
//  25..27  key_count         }
//  27..31  next_page_id      }
//  31..33  fsp_index         }
//  33+     slot array        }
//
// INDEX_HEADER_SIZE = 33  (slot array starts at byte 33)
//
// The write_next_free / read_next_free helpers intentionally still use bytes
// 0..4 (page_id range).  These are written only while the page sits on the
// B+ tree free list — i.e. before init() is called and before the page holds
// any index data — and are read back before init() clobbers them.  The
// BufferPool also writes page_id to 0..4, but the free-list pointer is read
// immediately after allocation so the values are consistent.
// ─────────────────────────────────────────────────────────────────────────────

/// Byte offset where the IndexPage-specific header begins.
/// Must be ≥ HEADER_SIZE (24) so the BufferPool's TablePage writes
/// (page_id, page_lsn, checksum, slot_count, fsp, page_type, flags)
/// never collide with IndexPage data.
const INDEX_HEADER_SIZE: usize = 33; // 24 (TablePage header) + 1 + 2 + 4 + 2
const INDEX_SLOT_SIZE: usize = 6; // 2 (payload_offset) + 2 (key_len) + 2 (value_len)

// Byte offsets for IndexPage-specific fields (all ≥ 24).
const OFF_IS_LEAF: usize = 24;
const OFF_KEY_COUNT: usize = 25; // ..27
const OFF_NEXT_PAGE_ID: usize = 27; // ..31
const OFF_FSP_INDEX: usize = 31; // ..33

pub struct IndexPage<T = [u8; PAGE_SIZE]> {
    pub(crate) data: T,
}

impl IndexPage<[u8; PAGE_SIZE]> {
    pub fn new(is_leaf: bool, next_page_id: u32) -> Self {
        let mut page = Self {
            data: [0u8; PAGE_SIZE],
        };
        page.set_is_leaf(is_leaf);
        page.set_key_count(0);
        page.set_next_page_id(next_page_id);
        page.set_fsp_index(PAGE_SIZE as u16);
        page
    }

    /// Converts a reference to a TablePage to a reference to an IndexPage.
    ///
    /// This is a completely safe conversion that returns an IndexPage view
    /// borrowing from the underlying page's data.
    pub fn from_page_ref<'a, U: AsRef<[u8]>>(page: &'a TablePage<U>) -> IndexPage<&'a [u8]> {
        IndexPage {
            data: page.data.as_ref(),
        }
    }

    /// Converts a mutable reference to a TablePage to a mutable reference to an IndexPage.
    ///
    /// This is a completely safe conversion that returns an IndexPage view
    /// borrowing from the underlying page's data mutably.
    pub fn from_page_mut<'a, U: AsMut<[u8]>>(
        page: &'a mut TablePage<U>,
    ) -> IndexPage<&'a mut [u8]> {
        IndexPage {
            data: page.data.as_mut(),
        }
    }
}

impl<T> IndexPage<T> {
    /// Creates a view of a page wrapping existing storage (e.g. `&[u8]` or `&mut [u8]`).
    pub fn new_view(data: T) -> Self {
        Self { data }
    }
}

impl<T: AsRef<[u8]>> IndexPage<T> {
    pub fn is_leaf(&self) -> bool {
        self.data.as_ref()[OFF_IS_LEAF] != 0
    }

    pub fn key_count(&self) -> u16 {
        u16::from_le_bytes(
            self.data.as_ref()[OFF_KEY_COUNT..OFF_KEY_COUNT + 2]
                .try_into()
                .unwrap(),
        )
    }

    pub fn next_page_id(&self) -> u32 {
        u32::from_le_bytes(
            self.data.as_ref()[OFF_NEXT_PAGE_ID..OFF_NEXT_PAGE_ID + 4]
                .try_into()
                .unwrap(),
        )
    }

    fn free_space_pointer(&self) -> u16 {
        u16::from_le_bytes(
            self.data.as_ref()[OFF_FSP_INDEX..OFF_FSP_INDEX + 2]
                .try_into()
                .unwrap(),
        )
    }

    pub fn free_space(&self) -> usize {
        let slot_array_end = INDEX_HEADER_SIZE + (self.key_count() as usize * INDEX_SLOT_SIZE);
        self.free_space_pointer() as usize - slot_array_end
    }

    pub fn get_key(&self, slot_idx: u16) -> Option<&[u8]> {
        if slot_idx >= self.key_count() {
            return None;
        }

        let slot_off = INDEX_HEADER_SIZE + slot_idx as usize * INDEX_SLOT_SIZE;
        let payload_off = u16::from_le_bytes(
            self.data.as_ref()[slot_off..slot_off + 2]
                .try_into()
                .unwrap(),
        ) as usize;
        let key_len = u16::from_le_bytes(
            self.data.as_ref()[slot_off + 2..slot_off + 4]
                .try_into()
                .unwrap(),
        ) as usize;

        Some(&self.data.as_ref()[payload_off..payload_off + key_len])
    }

    pub fn get_value(&self, slot_idx: u16) -> Option<&[u8]> {
        if slot_idx >= self.key_count() {
            return None;
        }

        let slot_off = INDEX_HEADER_SIZE + slot_idx as usize * INDEX_SLOT_SIZE;
        let payload_off = u16::from_le_bytes(
            self.data.as_ref()[slot_off..slot_off + 2]
                .try_into()
                .unwrap(),
        ) as usize;
        let key_len = u16::from_le_bytes(
            self.data.as_ref()[slot_off + 2..slot_off + 4]
                .try_into()
                .unwrap(),
        ) as usize;
        let val_len = u16::from_le_bytes(
            self.data.as_ref()[slot_off + 4..slot_off + 6]
                .try_into()
                .unwrap(),
        ) as usize;

        let value_off = payload_off + key_len;
        Some(&self.data.as_ref()[value_off..value_off + val_len])
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

    pub fn read_next_free(&self) -> u32 {
        u32::from_le_bytes(self.data.as_ref()[0..4].try_into().unwrap())
    }
}

impl<T: AsRef<[u8]> + AsMut<[u8]>> IndexPage<T> {
    pub fn init(&mut self, is_leaf: bool, next_page_id: u32) {
        self.data.as_mut().fill(0);
        self.set_is_leaf(is_leaf);
        self.set_key_count(0);
        self.set_next_page_id(next_page_id);
        self.set_fsp_index(PAGE_SIZE as u16);
    }

    pub fn set_is_leaf(&mut self, is_leaf: bool) {
        self.data.as_mut()[OFF_IS_LEAF] = if is_leaf { 1 } else { 0 };
    }

    pub fn set_key_count(&mut self, count: u16) {
        self.data.as_mut()[OFF_KEY_COUNT..OFF_KEY_COUNT + 2].copy_from_slice(&count.to_le_bytes());
    }

    pub fn set_next_page_id(&mut self, id: u32) {
        self.data.as_mut()[OFF_NEXT_PAGE_ID..OFF_NEXT_PAGE_ID + 4]
            .copy_from_slice(&id.to_le_bytes());
    }

    fn set_fsp_index(&mut self, ptr: u16) {
        self.data.as_mut()[OFF_FSP_INDEX..OFF_FSP_INDEX + 2].copy_from_slice(&ptr.to_le_bytes());
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
            self.data.as_mut().copy_within(
                target_slot_off..current_slot_array_end,
                target_slot_off + INDEX_SLOT_SIZE,
            );
        }

        let new_fsp = self.free_space_pointer() as usize - payload_len;
        self.data.as_mut()[new_fsp..new_fsp + key.len()].copy_from_slice(key);
        self.data.as_mut()[new_fsp + key.len()..new_fsp + payload_len].copy_from_slice(value);

        self.data.as_mut()[target_slot_off..target_slot_off + 2]
            .copy_from_slice(&(new_fsp as u16).to_le_bytes());
        self.data.as_mut()[target_slot_off + 2..target_slot_off + 4]
            .copy_from_slice(&(key.len() as u16).to_le_bytes());
        self.data.as_mut()[target_slot_off + 4..target_slot_off + 6]
            .copy_from_slice(&(value.len() as u16).to_le_bytes());

        self.set_key_count(count + 1);
        self.set_fsp_index(new_fsp as u16);
        true
    }

    pub fn split_into<U: AsRef<[u8]> + AsMut<[u8]>>(
        &mut self,
        right_page: &mut IndexPage<U>,
    ) -> Vec<u8> {
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
            .as_mut()
            .copy_within(slot_off + INDEX_SLOT_SIZE..end, slot_off);
        self.set_key_count(count - 1);
        self.compact();
    }

    pub fn compact(&mut self) {
        let count = self.key_count() as usize;
        if count == 0 {
            self.set_key_count(0);
            self.set_fsp_index(PAGE_SIZE as u16);
            return;
        }

        let mut temp_buffer = [0u8; PAGE_SIZE];
        let mut temp_fsp = PAGE_SIZE;

        let mut new_offsets = vec![0u16; count];

        for i in 0..count {
            let slot_off = INDEX_HEADER_SIZE + i * INDEX_SLOT_SIZE;

            let old_payload_off = u16::from_le_bytes(
                self.data.as_ref()[slot_off..slot_off + 2]
                    .try_into()
                    .unwrap(),
            ) as usize;
            let key_len = u16::from_le_bytes(
                self.data.as_ref()[slot_off + 2..slot_off + 4]
                    .try_into()
                    .unwrap(),
            ) as usize;
            let val_len = u16::from_le_bytes(
                self.data.as_ref()[slot_off + 4..slot_off + 6]
                    .try_into()
                    .unwrap(),
            ) as usize;
            let payload_len = key_len + val_len;

            temp_fsp -= payload_len;

            temp_buffer[temp_fsp..temp_fsp + payload_len].copy_from_slice(
                &self.data.as_ref()[old_payload_off..old_payload_off + payload_len],
            );

            new_offsets[i] = temp_fsp as u16;
        }

        self.data.as_mut()[temp_fsp..PAGE_SIZE].copy_from_slice(&temp_buffer[temp_fsp..PAGE_SIZE]);

        for i in 0..count {
            let slot_off = INDEX_HEADER_SIZE + i * INDEX_SLOT_SIZE;
            self.data.as_mut()[slot_off..slot_off + 2]
                .copy_from_slice(&new_offsets[i].to_le_bytes());
        }

        self.set_fsp_index(temp_fsp as u16);
    }

    pub fn write_next_free(&mut self, next: u32) {
        self.data.as_mut()[0..4].copy_from_slice(&next.to_le_bytes());
    }
}

impl<T: AsRef<[u8]>> AsRef<[u8]> for IndexPage<T> {
    fn as_ref(&self) -> &[u8] {
        self.data.as_ref()
    }
}

impl<T: AsMut<[u8]>> AsMut<[u8]> for IndexPage<T> {
    fn as_mut(&mut self) -> &mut [u8] {
        self.data.as_mut()
    }
}
