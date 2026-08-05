use crate::storage::{
    HeapFile, StorageError,
    page::{
        TablePage,
        header::{HEADER_SIZE, PageType},
        raw_page::PAGE_SIZE,
    },
};

pub const OVERFLOW_HEADER_SIZE: usize = 12;
pub const MAX_OVERFLOW_PAYLOAD_PER_PAGE: usize = PAGE_SIZE - HEADER_SIZE - OVERFLOW_HEADER_SIZE; // 8156 bytes

/// Manages writing and reading multi-page TOAST payload chains in a `.toast` file.
pub struct ToastManager;

impl ToastManager {
    /// Writes a large byte slice into chained overflow pages in `toast_file`.
    ///
    /// Returns the `first_page_id` of the overflow chain.
    pub fn write_payload(toast_file: &mut HeapFile, payload: &[u8]) -> Result<u32, StorageError> {
        let total_len = payload.len() as u32;
        let mut offset = 0;
        let mut chunk_index = 0u16;

        let mut prev_page_id: Option<u32> = None;
        let mut first_page_id: Option<u32> = None;

        while offset < payload.len() {
            let chunk_size = (payload.len() - offset).min(MAX_OVERFLOW_PAYLOAD_PER_PAGE);
            let chunk_data = &payload[offset..offset + chunk_size];

            // Allocate a new page in the toast file
            let page_id = toast_file.allocate_page()?;
            if first_page_id.is_none() {
                first_page_id = Some(page_id);
            }

            // Create and construct the overflow page
            let mut page = TablePage::new(page_id);
            page.set_page_type(PageType::Overflow);

            let byte_ref = page.as_bytes_mut();

            // Write Overflow Chunk Header (12 bytes starting at byte offset 24)
            let hdr_off = HEADER_SIZE;
            byte_ref[hdr_off..hdr_off + 4].copy_from_slice(&total_len.to_le_bytes());
            byte_ref[hdr_off + 4..hdr_off + 6].copy_from_slice(&(chunk_size as u16).to_le_bytes());
            byte_ref[hdr_off + 6..hdr_off + 8].copy_from_slice(&chunk_index.to_le_bytes());
            // Default next_page_id to u32::MAX (end of chain)
            byte_ref[hdr_off + 8..hdr_off + 12].copy_from_slice(&u32::MAX.to_le_bytes());

            // Write chunk data
            let payload_off = hdr_off + OVERFLOW_HEADER_SIZE;
            byte_ref[payload_off..payload_off + chunk_size].copy_from_slice(chunk_data);

            // Write page to disk
            toast_file.write_page(page_id, &page)?;

            // Link previous page's nex_page_id to this new page_id
            if let Some(prev_id) = prev_page_id {
                let mut prev_page = toast_file.read_page(prev_id)?;
                let prev_bytes = prev_page.as_bytes_mut();
                prev_bytes[hdr_off + 8..hdr_off + 12].copy_from_slice(&page_id.to_le_bytes());
                toast_file.write_page(prev_id, &prev_page)?;
            }

            prev_page_id = Some(page_id);
            offset += chunk_size;
            chunk_index += 1;
        }

        first_page_id.ok_or_else(|| StorageError::TupleError("empty TOAST payload".into()))
    }

    /// Reads and reassembles a full TOAST payload by traversing the page chain starting at `first_page_id`.
    pub fn read_payload(
        toast_file: &mut HeapFile,
        first_page_id: u32,
    ) -> Result<Vec<u8>, StorageError> {
        let mut result = Vec::new();
        let mut curr_page_id = first_page_id;

        loop {
            let page = toast_file.read_page(curr_page_id)?;
            let bytes = page.as_bytes();

            let hdr_off = HEADER_SIZE;
            let chunk_len =
                u16::from_le_bytes(bytes[hdr_off + 4..hdr_off + 6].try_into().unwrap()) as usize;
            let next_page_id =
                u32::from_le_bytes(bytes[hdr_off + 8..hdr_off + 12].try_into().unwrap());

            let payload_off = hdr_off + OVERFLOW_HEADER_SIZE;
            result.extend_from_slice(&bytes[payload_off..payload_off + chunk_len]);

            if next_page_id == u32::MAX {
                break; // End of chain
            }
            curr_page_id = next_page_id;
        }

        Ok(result)
    }
}
