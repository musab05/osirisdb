#[cfg(test)]
mod tests {
    use osirisdb::ast::{DataType, Value};
    use osirisdb::catalog::objects::ColumnEntry;
    use osirisdb::common::Interner;
    use osirisdb::storage::tuple::tuple::{deserialize_tuple, serialize_tuple};
    use osirisdb::storage::{BufferPool, Storage, StorageError, TableHeap, file::HeapFile};
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
        let wal_path = path.to_path_buf();
        let mut os = wal_path.into_os_string();
        os.push(".wal");
        let _ = std::fs::remove_file(os);
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
        hf.write_page(page_id, &page).unwrap();

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
            hf.write_page(page_id, &page).unwrap();
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
        hf.write_page(p0, &page0).unwrap();
        hf.write_page(p1, &page1).unwrap();

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

    #[test]
    fn table_heap_insert_and_scan() {
        let path = env::temp_dir().join("osirisdb_th_insert_and_scan");
        if path.exists() {
            let _ = std::fs::remove_dir_all(&path);
        }

        let storage = Storage::new_or_create(&path).unwrap();
        std::fs::create_dir_all(storage.schema_path("test_db", "test_schema")).unwrap();

        let mut th = TableHeap::open(&storage, "test_db", "test_schema", "test_table").unwrap();
        let mut interner = Interner::new();

        let schema = vec![
            col(&mut interner, "id", DataType::Int, false),
            col(&mut interner, "name", DataType::VarChar(None), false),
        ];

        let name1 = interner.intern("alice");
        let row1 = vec![Value::Int(1), Value::String(name1)];

        let name2 = interner.intern("bob");
        let row2 = vec![Value::Int(2), Value::String(name2)];

        let name3 = interner.intern("charlie");
        let row3 = vec![Value::Int(3), Value::String(name3)];

        th.insert_tuple(&schema, &row1, &interner, None).unwrap();
        th.insert_tuple(&schema, &row2, &interner, None).unwrap();
        th.insert_tuple(&schema, &row3, &interner, None).unwrap();

        let scanned = th.scan(&schema, &mut interner).unwrap();

        assert_eq!(scanned.len(), 3);
        assert_eq!(scanned[0], row1);
        assert_eq!(scanned[1], row2);
        assert_eq!(scanned[2], row3);

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn crc32c_checksum_verification_and_corruption_detection() {
        use osirisdb::storage::page::TablePage;

        let mut page = TablePage::new(1);
        page.insert_tuple(b"checksum payload test").unwrap();
        page.compute_checksum();

        // Verification should succeed on an uncorrupted page
        assert!(page.verify_checksum());

        // Corrupt a byte in the tuple area
        let bytes = page.as_bytes_mut();
        bytes[200] ^= 0xFF;

        // Verification must now fail
        assert!(!page.verify_checksum());
    }

    #[test]
    fn table_page_fragmentation_and_compaction() {
        use osirisdb::storage::page::{PageFlags, TablePage};

        let mut page = TablePage::new(0);
        let s0 = page.insert_tuple(b"row zero").unwrap();
        let s1 = page.insert_tuple(b"row one to be deleted").unwrap();
        let s2 = page.insert_tuple(b"row two").unwrap();

        assert_eq!(page.fragmented_space(), 0);

        // Delete row 1 -> leaves fragmented space
        assert!(page.delete_tuple(s1));
        assert!(page.fragmented_space() > 0);

        // Compact the page
        page.compact();
        assert_eq!(page.fragmented_space(), 0);
        assert!(PageFlags::is_set(page.flags(), PageFlags::COMPACT));

        // Surviving rows should still be accessible
        assert_eq!(page.get_tuple(s0), Some(&b"row zero"[..]));
        assert_eq!(page.get_tuple(s1), None);
        assert_eq!(page.get_tuple(s2), Some(&b"row two"[..]));
    }

    #[test]
    fn table_heap_vacuum_reclaims_defragmented_bytes() {
        let path = env::temp_dir().join("osirisdb_th_vacuum_test");
        if path.exists() {
            let _ = std::fs::remove_dir_all(&path);
        }

        let storage = Storage::new_or_create(&path).unwrap();
        std::fs::create_dir_all(storage.schema_path("test_db", "test_schema")).unwrap();

        let mut th = TableHeap::open(&storage, "test_db", "test_schema", "test_table").unwrap();
        let mut interner = Interner::new();

        let schema = vec![
            col(&mut interner, "id", DataType::Int, false),
            col(&mut interner, "val", DataType::VarChar(None), false),
        ];

        let str_sym = interner.intern("temporary row data");
        let row = vec![Value::Int(1), Value::String(str_sym)];

        // Insert tuples
        th.insert_tuple(&schema, &row, &interner, None).unwrap();
        th.insert_tuple(&schema, &row, &interner, None).unwrap();

        // Vacuum on a page with no deleted tuples reclaims 0 bytes
        let reclaimed = th.vacuum().unwrap();
        assert_eq!(reclaimed, 0);

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn toast_large_string_round_trip() {
        use osirisdb::storage::tuple::tuple::{
            deserialize_tuple_with_toast, serialize_tuple_with_toast,
        };

        let path = env::temp_dir().join("osirisdb_toast_test");
        if path.exists() {
            let _ = std::fs::remove_dir_all(&path);
        }

        let storage = Storage::new_or_create(&path).unwrap();
        std::fs::create_dir_all(storage.schema_path("test_db", "test_schema")).unwrap();

        let mut th = TableHeap::open(&storage, "test_db", "test_schema", "toast_table").unwrap();
        let mut interner = Interner::new();

        let schema = vec![
            col(&mut interner, "id", DataType::Int, false),
            col(&mut interner, "big_text", DataType::Text, false),
        ];

        // Create a large string exceeding TOAST threshold (3000 bytes)
        let large_string = "A".repeat(3000);
        let large_sym = interner.intern(&large_string);
        let row = vec![Value::Int(100), Value::String(large_sym)];

        let toast_file = th
            .get_or_open_toast_file(&storage, "test_db", "test_schema", "toast_table")
            .unwrap();

        let (bytes, has_toast) =
            serialize_tuple_with_toast(&schema, &row, &interner, Some(toast_file)).unwrap();
        assert!(has_toast);
        // The inline tuple size should now be very small (NULL bitmap + 8B int + 10B ToastPointer)
        assert!(bytes.len() < 100);

        // Deserializing with the toast file should reassemble the full 3000-byte string
        let toast_file_read = th
            .get_or_open_toast_file(&storage, "test_db", "test_schema", "toast_table")
            .unwrap();
        let decoded =
            deserialize_tuple_with_toast(&schema, &bytes, &interner, Some(toast_file_read))
                .unwrap();

        assert_eq!(decoded[0], Value::Int(100));
        if let Value::String(sym) = &decoded[1] {
            assert_eq!(interner.resolve(sym.clone()), large_string);
        } else {
            panic!("expected string value");
        }

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn table_heap_with_log_manager_assigns_page_lsn() {
        use osirisdb::storage::log::log_manager::LogManager;
        use std::sync::Arc;

        let path = env::temp_dir().join("osirisdb_th_wal_test");
        if path.exists() {
            let _ = std::fs::remove_dir_all(&path);
        }

        let storage = Storage::new_or_create(&path).unwrap();
        std::fs::create_dir_all(storage.schema_path("test_db", "test_schema")).unwrap();

        let log_path = path.join("test_wal.log");
        let log_manager = Arc::new(LogManager::new(&log_path).unwrap());

        let mut th = TableHeap::open_with_log_manager(
            &storage,
            "test_db",
            "test_schema",
            "users",
            Arc::clone(&log_manager),
        )
        .unwrap();

        let mut interner = Interner::new();
        let schema = vec![
            col(&mut interner, "id", DataType::Int, false),
            col(&mut interner, "name", DataType::VarChar(Some(50)), false),
        ];

        let alice = interner.intern("Alice");
        let row1 = vec![Value::Int(1), Value::String(alice)];
        let (page_id, slot_id) = th.insert_tuple(&schema, &row1, &interner, None).unwrap();

        assert_eq!(page_id, 0);
        assert_eq!(slot_id, 0);

        // Access the buffer pool to verify page_lsn was updated
        let bp_handle = th.buffer_pool_handle();
        let mut bp = bp_handle.lock().unwrap();
        let frame_id = bp.pin_page(page_id).unwrap();
        let page = bp.get_page(frame_id);

        // page_lsn must be 1 (first assigned LSN)
        assert_eq!(page.page_lsn(), 1);
        bp.unpin_page(frame_id, false);
        drop(bp);

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn buffer_pool_eviction_enforces_wal_flush_rule() {
        use osirisdb::storage::log::log_manager::LogManager;
        use osirisdb::storage::log::log_record::{LogRecord, RecordType};
        use std::sync::Arc;

        let p1 = tmp("bp_wal_evict_1");
        let p2 = tmp("bp_wal_evict_2");
        let log_p = tmp("bp_wal_evict.log");
        rm(&p1);
        rm(&p2);
        rm(&log_p);

        let log_manager = Arc::new(LogManager::new(&log_p).unwrap());
        let hf = HeapFile::open(&p1).unwrap();

        // Buffer pool with capacity 1
        let mut bp = BufferPool::with_log_manager(hf, 1, Arc::clone(&log_manager));

        // Allocate and pin page 0
        let (page_0_id, frame_0) = bp.new_page().unwrap();
        assert_eq!(page_0_id, 0);

        // Mutate page 0 and assign a higher LSN
        let mut dummy_record = LogRecord {
            lsn: 0,
            prev_lsn: 0,
            txt_id: 1,
            record_type: RecordType::Insert,
            file_id: 1,
            page_id: 0,
            offset: 0,
            length: 5,
            before_image: vec![],
            after_image: vec![1, 2, 3, 4, 5],
        };
        let assigned_lsn = log_manager.append_record(&mut dummy_record).unwrap();
        assert_eq!(assigned_lsn.0, 1);
        assert_eq!(log_manager.get_flushed_lsn(), 0); // Not flushed yet

        let page = bp.get_page_mut(frame_0);
        page.set_page_lsn(assigned_lsn.0);
        page.insert_tuple(b"data").unwrap();
        bp.unpin_page(frame_0, true);

        // Now allocate page 1 — since capacity is 1, this FORCES eviction of page 0
        let (page_1_id, _) = bp.new_page().unwrap();
        assert_eq!(page_1_id, 1);

        // The WAL Rule must have forced LogManager to flush page 0's LSN before writing it to disk
        assert!(log_manager.get_flushed_lsn() >= assigned_lsn.0);

        rm(&p1);
        rm(&p2);
        rm(&log_p);
    }

    #[test]
    fn buffer_pool_flush_all_enforces_wal_flush_rule() {
        use osirisdb::storage::log::log_manager::LogManager;
        use osirisdb::storage::log::log_record::{LogRecord, RecordType};
        use std::sync::Arc;

        let p = tmp("bp_wal_flush_all");
        let log_p = tmp("bp_wal_flush_all.log");
        rm(&p);
        rm(&log_p);

        let log_manager = Arc::new(LogManager::new(&log_p).unwrap());
        let hf = HeapFile::open(&p).unwrap();

        let mut bp = BufferPool::with_log_manager(hf, 4, Arc::clone(&log_manager));

        let (page_id, frame_id) = bp.new_page().unwrap();

        let mut dummy = LogRecord {
            lsn: 0,
            prev_lsn: 0,
            txt_id: 1,
            record_type: RecordType::Insert,
            file_id: 1,
            page_id,
            offset: 0,
            length: 4,
            before_image: vec![],
            after_image: vec![9, 8, 7, 6],
        };
        let lsn = log_manager.append_record(&mut dummy).unwrap();
        assert_eq!(log_manager.get_flushed_lsn(), 0);

        let page = bp.get_page_mut(frame_id);
        page.set_page_lsn(lsn.0);
        page.insert_tuple(b"test").unwrap();
        bp.unpin_page(frame_id, true);

        // Calling flush_all must enforce WAL flush before disk write
        bp.flush_all().unwrap();
        assert!(log_manager.get_flushed_lsn() >= lsn.0);

        rm(&p);
        rm(&log_p);
    }
}
