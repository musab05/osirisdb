#[cfg(test)]
mod tests {
    use osirisdb::storage::{StorageError, heap_file::HeapFile};
    use std::env;
    use std::path::{Path, PathBuf};

    /// Returns a unique temp-dir path for the named test.
    /// Using distinct names avoids collisions when tests run in parallel.
    fn tmp(name: &str) -> PathBuf {
        env::temp_dir().join(format!("osirisdb_hf_{}.dat", name))
    }

    /// Deletes the temp file — call at the start and end of each test
    /// so a failed previous run doesn't poison the next one.
    fn rm(path: &Path) {
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn fresh_file_has_zero_pages() {
        let p = tmp("fresh");
        rm(&p);
        let hf = HeapFile::open(&p).unwrap();
        assert_eq!(hf.num_pages, 0);
        rm(&p);
    }

    #[test]
    fn allocate_increments_num_pages() {
        let p = tmp("alloc");
        rm(&p);
        let mut hf = HeapFile::open(&p).unwrap();
        assert_eq!(hf.allocate_page().unwrap(), 0);
        assert_eq!(hf.allocate_page().unwrap(), 1);
        assert_eq!(hf.allocate_page().unwrap(), 2);
        assert_eq!(hf.num_pages, 3);
        rm(&p);
    }

    #[test]
    fn write_and_read_round_trip() {
        let p = tmp("roundtrip");
        rm(&p);
        let mut hf = HeapFile::open(&p).unwrap();
        let page_id = hf.allocate_page().unwrap();

        let mut page = hf.read_page(page_id).unwrap();
        let slot = page.insert_tuple(b"hello from disk").unwrap();
        hf.write_page(&page).unwrap();

        let page2 = hf.read_page(page_id).unwrap();
        assert_eq!(page2.get_tuple(slot), Some(&b"hello from disk"[..]));
        rm(&p);
    }

    #[test]
    fn data_persists_across_reopen() {
        let p = tmp("persist");
        rm(&p);

        let slot;
        {
            let mut hf = HeapFile::open(&p).unwrap();
            let page_id = hf.allocate_page().unwrap();
            let mut page = hf.read_page(page_id).unwrap();
            slot = page.insert_tuple(b"persisted tuple").unwrap();
            hf.write_page(&page).unwrap();
        } // file handle dropped / closed here

        {
            let mut hf = HeapFile::open(&p).unwrap();
            assert_eq!(hf.num_pages, 1); // recomputed from file length on reopen
            let page = hf.read_page(0).unwrap();
            assert_eq!(page.get_tuple(slot), Some(&b"persisted tuple"[..]));
        }

        rm(&p);
    }

    #[test]
    fn read_out_of_bounds_errors() {
        let p = tmp("oob");
        rm(&p);
        let mut hf = HeapFile::open(&p).unwrap();
        assert!(matches!(
            hf.read_page(0),
            Err(StorageError::PageOutOfBounds { .. })
        ));
        rm(&p);
    }

    #[test]
    fn multiple_pages_are_independent() {
        let p = tmp("multi");
        rm(&p);
        let mut hf = HeapFile::open(&p).unwrap();
        let p0 = hf.allocate_page().unwrap();
        let p1 = hf.allocate_page().unwrap();

        let mut page0 = hf.read_page(p0).unwrap();
        let mut page1 = hf.read_page(p1).unwrap();
        let s0 = page0.insert_tuple(b"page zero").unwrap();
        let s1 = page1.insert_tuple(b"page one").unwrap();
        hf.write_page(&page0).unwrap();
        hf.write_page(&page1).unwrap();

        assert_eq!(
            hf.read_page(p0).unwrap().get_tuple(s0),
            Some(&b"page zero"[..])
        );
        assert_eq!(
            hf.read_page(p1).unwrap().get_tuple(s1),
            Some(&b"page one"[..])
        );
        rm(&p);
    }
}
