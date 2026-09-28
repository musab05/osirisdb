use std::{
    env::temp_dir,
    fs,
    sync::{Arc, Mutex},
};

use osirisdb::{
    ast::{DataType, Value},
    catalog::objects::ColumnEntry,
    common::interner::Interner,
    storage::{
        BufferPool, Clog, ClogStatus, FileRegistry, LogManager, RecordId, Snapshot, Storage,
        TableHeap, TransactionManager, TupleHeader, TupleInfoMask, is_tuple_visible,
    },
};

#[test]
fn test_clog_status_transitions_and_bit_packing() {
    let clog = Clog::new();

    // Bootstrap transaction 0 is always considered committed
    assert_eq!(clog.get_status(0), ClogStatus::Committed);

    // Default status for unwritten transactions is InProgress
    assert_eq!(clog.get_status(1), ClogStatus::InProgress);
    assert_eq!(clog.get_status(2), ClogStatus::InProgress);
    assert_eq!(clog.get_status(3), ClogStatus::InProgress);
    assert_eq!(clog.get_status(4), ClogStatus::InProgress);

    // Set statuses for transactions sharing the same byte (1, 2, 3) and next byte (4)
    clog.set_status(1, ClogStatus::Committed);
    clog.set_status(2, ClogStatus::Aborted);
    clog.set_status(3, ClogStatus::InProgress);
    clog.set_status(4, ClogStatus::Committed);

    // Verify bit-packing didn't corrupt neighbor transactions
    assert_eq!(clog.get_status(1), ClogStatus::Committed);
    assert_eq!(clog.get_status(2), ClogStatus::Aborted);
    assert_eq!(clog.get_status(3), ClogStatus::InProgress);
    assert_eq!(clog.get_status(4), ClogStatus::Committed);

    // Test a high transaction ID (triggers buffer resizing)
    clog.set_status(10_000, ClogStatus::Committed);
    assert_eq!(clog.get_status(10_000), ClogStatus::Committed);
    assert_eq!(clog.get_status(9_999), ClogStatus::InProgress);
}

#[test]
fn test_transaction_manager_clog_integration() {
    let log_path = temp_dir().join("test_clog_tm.log");
    let lm = Arc::new(LogManager::new(&log_path).unwrap());
    let file_registry = Arc::new(FileRegistry::open_or_create(&temp_dir()).unwrap());
    let buffer_pool = Arc::new(Mutex::new(BufferPool::new(64)));
    let tm = TransactionManager::new(lm, file_registry, buffer_pool);

    let mut txn1 = tm.begin().unwrap();
    let mut txn2 = tm.begin().unwrap();
    let txn3 = tm.begin().unwrap();

    // All active transactions must be marked InProgress in CLOG
    assert_eq!(tm.clog().get_status(txn1.txn_id), ClogStatus::InProgress);
    assert_eq!(tm.clog().get_status(txn2.txn_id), ClogStatus::InProgress);
    assert_eq!(tm.clog().get_status(txn3.txn_id), ClogStatus::InProgress);

    // Commit txn1
    tm.commit(&mut txn1).unwrap();
    assert_eq!(tm.clog().get_status(txn1.txn_id), ClogStatus::Committed);

    // Abort txn2
    tm.abort(&mut txn2).unwrap();
    assert_eq!(tm.clog().get_status(txn2.txn_id), ClogStatus::Aborted);

    // txn3 still in progress
    assert_eq!(tm.clog().get_status(txn3.txn_id), ClogStatus::InProgress);

    let _ = fs::remove_file(log_path);
}

#[test]
fn test_snapshot_active_and_future_checks() {
    let log_path = temp_dir().join("test_snapshot.log");
    let lm = Arc::new(LogManager::new(&log_path).unwrap());
    let file_registry = Arc::new(FileRegistry::open_or_create(&temp_dir()).unwrap());
    let buffer_pool = Arc::new(Mutex::new(BufferPool::new(64)));
    let tm = TransactionManager::new(lm, file_registry, buffer_pool);

    let mut txn1 = tm.begin().unwrap(); // id = 1
    let mut txn2 = tm.begin().unwrap(); // id = 2
    let _txn3 = tm.begin().unwrap(); // id = 3
    let _txn4 = tm.begin().unwrap(); // id = 4

    // Finish 1 and 2
    tm.commit(&mut txn1).unwrap();
    tm.abort(&mut txn2).unwrap();

    // Active transactions at snapshot time: 3, 4
    let snap = tm.take_snapshot();
    assert_eq!(snap.xmin, 3);
    assert_eq!(snap.xmax, 5);
    assert_eq!(snap.xip_list, vec![3, 4]);

    // Already completed before snapshot -> not active
    assert!(!snap.is_active(1));
    assert!(!snap.is_active(2));

    // In-flight when snapshot was taken -> active
    assert!(snap.is_active(3));
    assert!(snap.is_active(4));

    // Started after snapshot -> active (invisible)
    assert!(snap.is_active(5));
    assert!(snap.is_active(100));

    let _ = fs::remove_file(log_path);
}

#[test]
fn test_visibility_rules_and_hint_bits() {
    let clog = Clog::new();
    clog.set_status(10, ClogStatus::Committed);
    clog.set_status(20, ClogStatus::Aborted);
    clog.set_status(30, ClogStatus::Committed);
    clog.set_status(40, ClogStatus::Committed);

    let snapshot = Snapshot {
        xmin: 25,
        xmax: 50,
        xip_list: vec![30], // tx 30 was concurrent / in-flight
    };

    // Case 1: Self-inserted and not deleted
    let mut h1 = TupleHeader::new(
        99,
        0,
        RecordId {
            page_id: 1,
            slot_id: 0,
        },
    );
    assert!(is_tuple_visible(&mut h1, &snapshot, &clog, 99));

    // Case 2: Self-inserted and self-deleted
    let mut h2 = TupleHeader::new(
        99,
        0,
        RecordId {
            page_id: 1,
            slot_id: 1,
        },
    );
    h2.mark_deleted(99);
    assert!(!is_tuple_visible(&mut h2, &snapshot, &clog, 99));

    // Case 3: Created by aborted transaction 20 -> Invisible
    let mut h3 = TupleHeader::new(
        20,
        0,
        RecordId {
            page_id: 1,
            slot_id: 2,
        },
    );
    assert!(!is_tuple_visible(&mut h3, &snapshot, &clog, 100));
    assert!(TupleInfoMask::is_set(
        h3.infomask,
        TupleInfoMask::XMIN_ABORTED
    ));

    // Case 4: Created by committed txn 10 (finished before snapshot xmin=25) -> Visible
    let mut h4 = TupleHeader::new(
        10,
        0,
        RecordId {
            page_id: 1,
            slot_id: 3,
        },
    );
    assert!(is_tuple_visible(&mut h4, &snapshot, &clog, 100));
    // Verify hint bit was set
    assert!(TupleInfoMask::is_set(
        h4.infomask,
        TupleInfoMask::XMIN_COMMITTED
    ));

    // Case 5: Created by txn 30 which was in-flight at snapshot time -> Invisible
    let mut h5 = TupleHeader::new(
        30,
        0,
        RecordId {
            page_id: 1,
            slot_id: 4,
        },
    );
    assert!(!is_tuple_visible(&mut h5, &snapshot, &clog, 100));

    // Case 6: Created by txn 10, deleted by aborted txn 20 -> Visible (deletion rolled back)
    let mut h6 = TupleHeader::new(
        10,
        0,
        RecordId {
            page_id: 1,
            slot_id: 5,
        },
    );
    h6.mark_deleted(20);
    assert!(is_tuple_visible(&mut h6, &snapshot, &clog, 100));

    // Case 7: Created by txn 10, deleted by concurrent txn 30 -> Visible (deleted after our snapshot)
    let mut h7 = TupleHeader::new(
        10,
        0,
        RecordId {
            page_id: 1,
            slot_id: 6,
        },
    );
    h7.mark_deleted(30);
    assert!(is_tuple_visible(&mut h7, &snapshot, &clog, 100));
}

#[test]
fn test_table_heap_tuple_header_roundtrip() {
    let interner = Interner::new();
    let col_id = ColumnEntry {
        name: interner.intern("id"),
        data_type: DataType::Int,
        nullable: false,
        default: None,
        is_unique: false,
        is_primary_key: true,
    };
    let col_name = ColumnEntry {
        name: interner.intern("name"),
        data_type: DataType::VarChar(Some(50)),
        nullable: false,
        default: None,
        is_unique: false,
        is_primary_key: false,
    };
    let schema = vec![col_id, col_name];

    let path = temp_dir().join("test_th_mvcc_db");
    let _ = fs::remove_dir_all(&path);
    let storage = Storage::new_or_create(&path).unwrap();
    fs::create_dir_all(storage.schema_path("test_db", "test_schema")).unwrap();

    let mut th = TableHeap::open(&storage, "test_db", "test_schema", "mvcc_table").unwrap();

    let name = interner.intern("Alice");
    let row = vec![Value::Int(42), Value::String(name)];

    // Insert row
    let (pid, sid) = th
        .insert_tuple(&schema, &row, &interner, None, None)
        .unwrap();

    // Verify row can be read back via get_tuple
    let fetched = th
        .get_tuple(
            RecordId {
                page_id: pid,
                slot_id: sid,
            },
            &schema,
            &interner,
        )
        .unwrap();
    assert_eq!(fetched, Some(row.clone()));

    // Verify row can be scanned
    let scanned = th.scan(&schema, &interner).unwrap();
    assert_eq!(scanned.len(), 1);
    assert_eq!(scanned[0], row);

    let _ = fs::remove_dir_all(&path);
}

#[test]
fn test_deletion_visibility_rules() {
    let clog = Clog::new();
    clog.set_status(10, ClogStatus::Committed); // Creator committed
    clog.set_status(20, ClogStatus::Committed); // Deleter committed before snapshot
    clog.set_status(30, ClogStatus::InProgress); // Deleter still in progress
    clog.set_status(40, ClogStatus::Committed); // Deleter committed after snapshot
    clog.set_status(50, ClogStatus::Aborted); // Deleter aborted

    let snapshot = Snapshot {
        xmin: 25,
        xmax: 50,
        xip_list: vec![40], // tx 40 was in-flight when snapshot was taken
    };

    // Case 1: Deleted before snapshot by committed tx 20 -> Invisible
    let mut h1 = TupleHeader::new(
        10,
        0,
        RecordId {
            page_id: 1,
            slot_id: 1,
        },
    );
    h1.mark_deleted(20);
    assert!(!is_tuple_visible(&mut h1, &snapshot, &clog, 100));
    assert!(TupleInfoMask::is_set(
        h1.infomask,
        TupleInfoMask::XMAX_COMMITTED
    ));

    // Case 2: Deleted by in-progress tx 30 -> Still visible to snapshot
    let mut h2 = TupleHeader::new(
        10,
        0,
        RecordId {
            page_id: 1,
            slot_id: 2,
        },
    );
    h2.mark_deleted(30);
    assert!(is_tuple_visible(&mut h2, &snapshot, &clog, 100));

    // Case 3: Deleted by tx 40 which was concurrent/in-flight -> Still visible to snapshot
    let mut h3 = TupleHeader::new(
        10,
        0,
        RecordId {
            page_id: 1,
            slot_id: 3,
        },
    );
    h3.mark_deleted(40);
    assert!(is_tuple_visible(&mut h3, &snapshot, &clog, 100));

    // Case 4: Deleted by tx 60 which started after snapshot (xmax=50) -> Still visible
    let mut h4 = TupleHeader::new(
        10,
        0,
        RecordId {
            page_id: 1,
            slot_id: 4,
        },
    );
    h4.mark_deleted(60);
    assert!(is_tuple_visible(&mut h4, &snapshot, &clog, 100));

    // Case 5: Deleted by current transaction -> Invisible to current transaction
    let mut h5 = TupleHeader::new(
        10,
        0,
        RecordId {
            page_id: 1,
            slot_id: 5,
        },
    );
    h5.mark_deleted(99);
    assert!(!is_tuple_visible(&mut h5, &snapshot, &clog, 99));

    // Case 6: Deleted by aborted transaction 50 -> Visible, hint bit XMAX_ABORTED set
    let mut h6 = TupleHeader::new(
        10,
        0,
        RecordId {
            page_id: 1,
            slot_id: 6,
        },
    );
    h6.mark_deleted(50);
    assert!(is_tuple_visible(&mut h6, &snapshot, &clog, 100));
    assert!(TupleInfoMask::is_set(
        h6.infomask,
        TupleInfoMask::XMAX_ABORTED
    ));

    // Case 7: Subsequent visibility check uses hint bit directly without CLOG change impact
    assert!(is_tuple_visible(&mut h6, &snapshot, &clog, 100));
}

#[test]
fn test_snapshot_boundaries_and_empty_active() {
    let log_path = temp_dir().join("test_snap_empty.log");
    let lm = Arc::new(LogManager::new(&log_path).unwrap());
    let file_registry = Arc::new(FileRegistry::open_or_create(&temp_dir()).unwrap());
    let buffer_pool = Arc::new(Mutex::new(BufferPool::new(64)));
    let tm = TransactionManager::new(lm, file_registry, buffer_pool);

    // No active transactions -> xmin == xmax == next_txn_id (1)
    let snap_empty = tm.take_snapshot();
    assert_eq!(snap_empty.xmin, 1);
    assert_eq!(snap_empty.xmax, 1);
    assert!(snap_empty.xip_list.is_empty());

    // tx < xmin is not active
    assert!(!snap_empty.is_active(0));
    // tx >= xmax is active (invisible/future)
    assert!(snap_empty.is_active(1));
    assert!(snap_empty.is_active(2));

    let _ = fs::remove_file(log_path);
}

#[test]
fn test_mvcc_table_heap_end_to_end_visibility() {
    let interner = Interner::new();
    let schema = vec![
        ColumnEntry {
            name: interner.intern("id"),
            data_type: DataType::Int,
            nullable: false,
            default: None,
            is_unique: false,
            is_primary_key: true,
        },
        ColumnEntry {
            name: interner.intern("val"),
            data_type: DataType::VarChar(Some(32)),
            nullable: false,
            default: None,
            is_unique: false,
            is_primary_key: false,
        },
    ];

    let dir = temp_dir().join("test_mvcc_e2e");
    let _ = fs::remove_dir_all(&dir);
    let storage = Storage::new_or_create(&dir).unwrap();
    fs::create_dir_all(storage.schema_path("mydb", "public")).unwrap();

    let log_path = dir.join("wal.log");
    let lm = Arc::new(LogManager::new(&log_path).unwrap());
    let file_registry = Arc::new(FileRegistry::open_or_create(&dir).unwrap());
    let buffer_pool = Arc::new(Mutex::new(BufferPool::new(64)));
    let tm = TransactionManager::new(Arc::clone(&lm), file_registry, Arc::clone(&buffer_pool));

    let mut th = TableHeap::open(&storage, "mydb", "public", "items").unwrap();

    // Start txn1 and txn2
    let mut txn1 = tm.begin().unwrap(); // id = 1
    let mut txn2 = tm.begin().unwrap(); // id = 2

    let val1 = vec![Value::Int(10), Value::String(interner.intern("item_1"))];
    let val2 = vec![Value::Int(20), Value::String(interner.intern("item_2"))];

    let (pid1, sid1) = th
        .insert_tuple(&schema, &val1, &interner, Some(&mut txn1), Some(&lm))
        .unwrap();
    let (pid2, sid2) = th
        .insert_tuple(&schema, &val2, &interner, Some(&mut txn2), Some(&lm))
        .unwrap();

    // Snapshot taken while txn1 and txn2 are active
    let snap_early = tm.take_snapshot();
    assert_eq!(snap_early.xmin, 1);
    assert_eq!(snap_early.xmax, 3);
    assert_eq!(snap_early.xip_list, vec![1, 2]);

    // Commit txn1, Abort txn2
    tm.commit(&mut txn1).unwrap();
    tm.abort(&mut txn2).unwrap();

    // Snapshot taken after txn1 committed and txn2 aborted
    let snap_later = tm.take_snapshot();
    assert_eq!(snap_later.xmin, 3);
    assert_eq!(snap_later.xmax, 3);
    assert!(snap_later.xip_list.is_empty());

    // Fetch raw pages and headers to verify visibility under both snapshots
    let bp = th.buffer_pool_handle();
    let mut bp_lock = bp.lock().unwrap();

    // Verify row 1 (txn1)
    let frame1 = bp_lock.pin_page(th.file_id(), pid1).unwrap();
    let raw_bytes1 = bp_lock.get_page(frame1).get_tuple(sid1).unwrap().to_vec();
    bp_lock.unpin_page(frame1, false);

    let mut header1 = TupleHeader::from_bytes(&raw_bytes1[..28]).unwrap();
    assert_eq!(header1.xmin, 1);

    // Row 1 under early snapshot -> invisible (txn1 was in-flight)
    assert!(!is_tuple_visible(&mut header1, &snap_early, tm.clog(), 100));

    // Row 1 under later snapshot -> visible!
    assert!(is_tuple_visible(&mut header1, &snap_later, tm.clog(), 100));
    assert!(TupleInfoMask::is_set(
        header1.infomask,
        TupleInfoMask::XMIN_COMMITTED
    ));

    // Verify row 2 (txn2)
    let frame2 = bp_lock.pin_page(th.file_id(), pid2).unwrap();
    let raw_bytes2 = bp_lock.get_page(frame2).get_tuple(sid2).unwrap().to_vec();
    bp_lock.unpin_page(frame2, false);

    let mut header2 = TupleHeader::from_bytes(&raw_bytes2[..28]).unwrap();
    assert_eq!(header2.xmin, 2);

    // Row 2 under later snapshot -> invisible (txn2 aborted)!
    assert!(!is_tuple_visible(&mut header2, &snap_later, tm.clog(), 100));
    assert!(TupleInfoMask::is_set(
        header2.infomask,
        TupleInfoMask::XMIN_ABORTED
    ));

    drop(bp_lock);
    drop(th);
    let _ = fs::remove_dir_all(&dir);
}
