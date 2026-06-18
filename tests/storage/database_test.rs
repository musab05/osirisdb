#[cfg(test)]
mod tests {
    use osirisdb::ast::{DataType, Value};
    use osirisdb::catalog::objects::ColumnEntry;
    use osirisdb::common::Interner;
    use osirisdb::storage::tuple::{deserialize_tuple, serialize_tuple};
    use osirisdb::storage::{BufferPool, StorageError, heap_file::HeapFile};
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

    fn make_pool(name: &str, capacity: usize) -> (BufferPool, PathBuf) {
        let path = tmp(name);
        rm(&path);
        let hf = HeapFile::open(&path).unwrap();
        (BufferPool::new(hf, capacity), path)
    }

    #[test]
    fn new_page_and_pin_unpin() {
        let (mut bp, path) = make_pool("new_page", 4);

        let (page_id, frame_id) = bp.new_page().unwrap();
        assert_eq!(page_id, 0);

        // Insert a tuple while the page is pinned.
        let slot = bp.get_page_mut(frame_id).insert_tuple(b"hello").unwrap();
        bp.unpin_page(frame_id, true);

        // Pin again and verify the tuple is still there (still in cache).
        let frame_id2 = bp.pin_page(page_id).unwrap();
        assert_eq!(bp.get_page(frame_id2).get_tuple(slot), Some(&b"hello"[..]));
        bp.unpin_page(frame_id2, false);

        rm(&path);
    }

    #[test]
    fn cache_hit_no_extra_disk_read() {
        let (mut bp, path) = make_pool("cache_hit", 4);

        let (page_id, frame_id) = bp.new_page().unwrap();
        bp.unpin_page(frame_id, false);

        // Pin the same page twice — both should return the same frame.
        let f1 = bp.pin_page(page_id).unwrap();
        let f2 = bp.pin_page(page_id).unwrap();
        assert_eq!(f1, f2);
        bp.unpin_page(f1, false);
        bp.unpin_page(f2, false);

        rm(&path);
    }

    #[test]
    fn dirty_page_flushed_on_eviction() {
        // Pool with only 1 frame — forces eviction when a second page is pinned.
        let (mut bp, path) = make_pool("evict", 1);

        // Allocate two pages but we only have 1 frame.
        let (p0, f0) = bp.new_page().unwrap();
        let slot = bp.get_page_mut(f0).insert_tuple(b"evict me").unwrap();
        bp.unpin_page(f0, true); // dirty — must be written on eviction

        // Pinning p1 forces eviction of p0 (the only unpinned frame).
        let (_p1, f1) = bp.new_page().unwrap();
        bp.unpin_page(f1, false);

        // Now pin p0 again — it must be reloaded from disk.
        let f0b = bp.pin_page(p0).unwrap();
        assert_eq!(
            bp.get_page(f0b).get_tuple(slot),
            Some(&b"evict me"[..]),
            "dirty page should have been written back to disk before eviction"
        );
        bp.unpin_page(f0b, false);

        rm(&path);
    }

    #[test]
    fn flush_all_writes_dirty_frames() {
        let (mut bp, path) = make_pool("flush", 4);

        let (page_id, frame_id) = bp.new_page().unwrap();
        let slot = bp
            .get_page_mut(frame_id)
            .insert_tuple(b"flush test")
            .unwrap();
        bp.unpin_page(frame_id, true);

        bp.flush_all().unwrap();

        // Reopen the heap file and verify the data is on disk.
        let mut hf2 = HeapFile::open(&path).unwrap();
        let page = hf2.read_page(page_id).unwrap();
        assert_eq!(page.get_tuple(slot), Some(&b"flush test"[..]));

        rm(&path);
    }

    #[test]
    fn buffer_pool_full_error_when_all_pinned() {
        // Pool with 2 frames; pin both without unpinning.
        let (mut bp, path) = make_pool("full", 2);

        let (_p0, _f0) = bp.new_page().unwrap(); // frame 0 pinned, not unpinned
        let (_p1, _f1) = bp.new_page().unwrap(); // frame 1 pinned, not unpinned

        // Trying to allocate a third page should fail — pool is full.
        assert!(matches!(bp.new_page(), Err(StorageError::BufferPoolFull)));

        rm(&path);
    }

    /// Builds a minimal `ColumnEntry` for testing — only `name` and
    /// `data_type` matter here; constraints are irrelevant to serialization.
    fn col(
        interner: &mut Interner,
        name: &str,
        data_type: DataType,
        nullable: bool,
    ) -> ColumnEntry {
        ColumnEntry {
            name: interner.intern(name),
            data_type,
            nullable,
            default: None,
            is_unique: false,
            is_primary_key: false,
        }
    }

    #[test]
    fn round_trip_integers_and_string() {
        let mut interner = Interner::new();
        let schema = vec![
            col(&mut interner, "id", DataType::Int, false),
            col(&mut interner, "name", DataType::VarChar(None), false),
            col(&mut interner, "score", DataType::BigInt, false),
        ];

        let name_sym = interner.intern("alice");
        let values = vec![Value::Int(42), Value::String(name_sym), Value::Int(9999)];

        let bytes = serialize_tuple(&schema, &values, &interner).unwrap();
        let decoded = deserialize_tuple(&schema, &bytes, &mut interner).unwrap();

        assert_eq!(decoded[0], Value::Int(42));
        assert_eq!(decoded[2], Value::Int(9999));
        // String round-trip: the symbol may differ in value from the original
        // (interning is idempotent so it should be the same), but resolving
        // it should give back the original string.
        if let Value::String(sym) = &decoded[1] {
            assert_eq!(interner.resolve(*sym), "alice");
        } else {
            panic!("expected String value");
        }
    }

    #[test]
    fn round_trip_boolean() {
        let mut interner = Interner::new();
        let schema = vec![col(&mut interner, "active", DataType::Boolean, false)];
        let values = vec![Value::Boolean(true)];

        let bytes = serialize_tuple(&schema, &values, &interner).unwrap();
        let decoded = deserialize_tuple(&schema, &bytes, &mut interner).unwrap();
        assert_eq!(decoded[0], Value::Boolean(true));
    }

    #[test]
    fn null_column_no_value_bytes() {
        let mut interner = Interner::new();
        let schema = vec![
            col(&mut interner, "id", DataType::Int, false),
            col(&mut interner, "email", DataType::VarChar(None), true), // nullable
        ];
        let values = vec![Value::Int(1), Value::Null];

        let bytes = serialize_tuple(&schema, &values, &interner).unwrap();

        // Bitmap = 1 byte (2 columns). Column 1 is NULL → bit 1 set = 0b00000010.
        assert_eq!(bytes[0], 0b00000010);

        let decoded = deserialize_tuple(&schema, &bytes, &mut interner).unwrap();
        assert_eq!(decoded[0], Value::Int(1));
        assert_eq!(decoded[1], Value::Null);
    }

    #[test]
    fn null_on_not_null_column_errors() {
        let mut interner = Interner::new();
        let schema = vec![col(&mut interner, "id", DataType::Int, false)]; // NOT NULL
        let values = vec![Value::Null];

        assert!(matches!(
            serialize_tuple(&schema, &values, &interner),
            Err(StorageError::TupleError(_))
        ));
    }

    #[test]
    fn wrong_column_count_errors() {
        let mut interner = Interner::new();
        let schema = vec![col(&mut interner, "id", DataType::Int, false)];
        let values = vec![Value::Int(1), Value::Int(2)]; // too many

        assert!(matches!(
            serialize_tuple(&schema, &values, &interner),
            Err(StorageError::TupleError(_))
        ));
    }

    #[test]
    fn type_mismatch_errors() {
        let mut interner = Interner::new();
        let schema = vec![col(&mut interner, "id", DataType::Int, false)];
        let values = vec![Value::Boolean(true)]; // wrong type for INT column

        assert!(matches!(
            serialize_tuple(&schema, &values, &interner),
            Err(StorageError::TupleError(_))
        ));
    }

    #[test]
    fn smallint_range_check() {
        let mut interner = Interner::new();
        let schema = vec![col(&mut interner, "x", DataType::SmallInt, false)];

        // Valid SMALLINT value.
        let ok = serialize_tuple(&schema, &[Value::Int(32767)], &interner);
        assert!(ok.is_ok());

        // Overflow.
        let overflow = serialize_tuple(&schema, &[Value::Int(32768)], &interner);
        assert!(matches!(overflow, Err(StorageError::TupleError(_))));
    }
}
