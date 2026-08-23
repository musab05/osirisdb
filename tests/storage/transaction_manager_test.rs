use std::{env, fs::File, io::Read, sync::Arc, thread};

use osirisdb::{
    ast::{DataType, Value},
    catalog::objects::ColumnEntry,
    common::Interner,
    storage::{
        FileRegistry, LogManager, RecordId, Storage, TableHeap, TransactionManager,
        log::log_record::{LogRecord, RecordType},
        txn::transaction::TxnStatus,
    },
};

fn col(interner: &mut Interner, name: &str, data_type: DataType, nullable: bool) -> ColumnEntry {
    let sym = interner.intern(name);
    ColumnEntry {
        name: sym,
        data_type,
        nullable,
        default: None,
        is_unique: false,
        is_primary_key: false,
    }
}

/// Helper to parse all sequential [`LogRecord`]s from a raw WAL file.
fn read_all_log_records(log_path: &std::path::Path) -> Vec<LogRecord> {
    let mut file = File::open(log_path).expect("Failed to open log file");
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .expect("Failed to read log file");

    let mut records = Vec::new();
    let mut cursor = 0;

    while cursor + 45 <= bytes.len() {
        let before_len =
            u32::from_le_bytes(bytes[cursor + 37..cursor + 41].try_into().unwrap()) as usize;
        if cursor + 45 + before_len > bytes.len() {
            break;
        }
        let after_len = u32::from_le_bytes(
            bytes[cursor + 41 + before_len..cursor + 45 + before_len]
                .try_into()
                .unwrap(),
        ) as usize;
        let record_size = 49 + before_len + after_len;
        if cursor + record_size > bytes.len() {
            break;
        }

        let rec_slice = &bytes[cursor..cursor + record_size];
        let record = LogRecord::deserialize(rec_slice).expect("Failed to deserialize LogRecord");
        records.push(record);
        cursor += record_size;
    }

    records
}

#[test]
fn test_transaction_begin_assigns_lsn_and_registers_att() {
    let dir = env::temp_dir().join("osirisdb_txn_test_begin");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let log_path = dir.join("wal.log");
    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let tm = TransactionManager::new(Arc::clone(&log_manager));

    // Initially, no transactions active
    assert_eq!(tm.active_txn_count(), 0);

    // Begin transaction 1
    let txn1 = tm.begin().unwrap();
    assert_eq!(txn1.txn_id, 1);
    assert_eq!(txn1.status, TxnStatus::Active);
    assert_eq!(txn1.last_lsn, 1); // First allocated LSN in WAL is 1
    assert_eq!(tm.active_txn_count(), 1);
    assert_eq!(tm.get_active_txn(1), Some(txn1.clone()));

    // Begin transaction 2
    let txn2 = tm.begin().unwrap();
    assert_eq!(txn2.txn_id, 2);
    assert_eq!(txn2.status, TxnStatus::Active);
    assert_eq!(txn2.last_lsn, 2); // Second allocated LSN in WAL is 2
    assert_eq!(tm.active_txn_count(), 2);
    assert_eq!(tm.get_active_txn(2), Some(txn2.clone()));

    drop(tm);
    drop(log_manager);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_transaction_prev_lsn_chain_begin_insert_commit() {
    let dir = env::temp_dir().join("osirisdb_txn_test_chain");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let storage = Storage::new_or_create(&dir).unwrap();
    std::fs::create_dir_all(storage.schema_path("shop_db", "public")).unwrap();

    let log_path = dir.join("wal.log");
    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let tm = Arc::new(TransactionManager::new(Arc::clone(&log_manager)));

    let mut th = TableHeap::open_with_log_manager(
        &storage,
        "shop_db",
        "public",
        "orders",
        Arc::clone(&log_manager),
    )
    .unwrap();

    let mut interner = Interner::new();
    let schema = vec![
        col(&mut interner, "order_id", DataType::Int, false),
        col(
            &mut interner,
            "customer",
            DataType::VarChar(Some(50)),
            false,
        ),
    ];

    // 1. BEGIN transaction
    let mut txn = tm.begin().unwrap();
    let txn_id = txn.txn_id;
    let begin_lsn = txn.last_lsn;
    assert_eq!(begin_lsn, 1);
    assert_eq!(tm.active_txn_count(), 1);

    // 2. INSERT first tuple
    let cust1 = interner.intern("Alice");
    let row1 = vec![Value::Int(101), Value::String(cust1)];
    let (p1, s1) = th
        .insert_tuple(&schema, &row1, &interner, Some(&mut txn))
        .unwrap();
    let insert1_lsn = txn.last_lsn;
    assert_eq!(insert1_lsn, 2);
    assert_eq!(p1, 0);
    assert_eq!(s1, 0);

    // 3. INSERT second tuple
    let cust2 = interner.intern("Bob");
    let row2 = vec![Value::Int(102), Value::String(cust2)];
    let (p2, s2) = th
        .insert_tuple(&schema, &row2, &interner, Some(&mut txn))
        .unwrap();
    let insert2_lsn = txn.last_lsn;
    assert_eq!(insert2_lsn, 3);
    assert_eq!(p2, 0);
    assert_eq!(s2, 1);

    // 4. COMMIT transaction
    tm.commit(&mut txn).unwrap();
    let commit_lsn = txn.last_lsn;
    assert_eq!(commit_lsn, 4);
    assert_eq!(txn.status, TxnStatus::Committed);

    // Active Transaction Table must now be empty
    assert_eq!(tm.active_txn_count(), 0);
    assert_eq!(tm.get_active_txn(txn_id), None);

    // Durability invariant: COMMIT forces WAL sync to disk (Group Commit)
    assert!(log_manager.get_flushed_lsn() >= commit_lsn);

    // Flush and release handles before reading disk file
    drop(th);
    drop(tm);
    drop(log_manager);

    // 5. Read physical WAL records from disk and verify backward prev_lsn chain:
    //    BEGIN (LSN 1, prev 0) <- INSERT1 (LSN 2, prev 1) <- INSERT2 (LSN 3, prev 2) <- COMMIT (LSN 4, prev 3)
    let records = read_all_log_records(&log_path);
    assert_eq!(records.len(), 4);

    // Record 0: BEGIN
    assert_eq!(records[0].record_type, RecordType::Begin);
    assert_eq!(records[0].lsn, begin_lsn);
    assert_eq!(records[0].prev_lsn, 0);
    assert_eq!(records[0].txt_id, txn_id);

    // Record 1: INSERT #1
    assert_eq!(records[1].record_type, RecordType::Insert);
    assert_eq!(records[1].lsn, insert1_lsn);
    assert_eq!(records[1].prev_lsn, begin_lsn);
    assert_eq!(records[1].txt_id, txn_id);
    assert_eq!(records[1].page_id, p1);
    assert_eq!(records[1].offset, s1);

    // Record 2: INSERT #2
    assert_eq!(records[2].record_type, RecordType::Insert);
    assert_eq!(records[2].lsn, insert2_lsn);
    assert_eq!(records[2].prev_lsn, insert1_lsn);
    assert_eq!(records[2].txt_id, txn_id);
    assert_eq!(records[2].page_id, p2);
    assert_eq!(records[2].offset, s2);

    // Record 3: COMMIT
    assert_eq!(records[3].record_type, RecordType::Commit);
    assert_eq!(records[3].lsn, commit_lsn);
    assert_eq!(records[3].prev_lsn, insert2_lsn);
    assert_eq!(records[3].txt_id, txn_id);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_transaction_abort_records_abort_and_removes_from_att() {
    let dir = env::temp_dir().join("osirisdb_txn_test_abort");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let storage = Storage::new_or_create(&dir).unwrap();
    std::fs::create_dir_all(storage.schema_path("shop_db", "public")).unwrap();

    let log_path = dir.join("wal.log");
    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let tm = Arc::new(TransactionManager::new(Arc::clone(&log_manager)));

    let mut th = TableHeap::open_with_log_manager(
        &storage,
        "shop_db",
        "public",
        "orders",
        Arc::clone(&log_manager),
    )
    .unwrap();

    let mut interner = Interner::new();
    let schema = vec![col(&mut interner, "id", DataType::Int, false)];

    // 1. BEGIN transaction
    let mut txn = tm.begin().unwrap();
    let txn_id = txn.txn_id;
    let begin_lsn = txn.last_lsn;
    assert_eq!(tm.active_txn_count(), 1);

    // 2. INSERT tuple
    let row = vec![Value::Int(999)];
    th.insert_tuple(&schema, &row, &interner, Some(&mut txn))
        .unwrap();
    let insert_lsn = txn.last_lsn;
    assert!(insert_lsn > begin_lsn);

    // 3. ABORT transaction
    tm.abort(&mut txn).unwrap();
    let abort_lsn = txn.last_lsn;
    assert!(abort_lsn > insert_lsn);
    assert_eq!(txn.status, TxnStatus::Aborted);

    // Active Transaction Table must now be empty
    assert_eq!(tm.active_txn_count(), 0);
    assert_eq!(tm.get_active_txn(txn_id), None);

    // Flush WAL to inspect physical log
    log_manager.flush().unwrap();
    drop(th);
    drop(tm);
    drop(log_manager);

    let records = read_all_log_records(&log_path);
    assert_eq!(records.len(), 3);

    // Record 0: BEGIN
    assert_eq!(records[0].record_type, RecordType::Begin);
    assert_eq!(records[0].lsn, begin_lsn);
    assert_eq!(records[0].prev_lsn, 0);

    // Record 1: INSERT
    assert_eq!(records[1].record_type, RecordType::Insert);
    assert_eq!(records[1].lsn, insert_lsn);
    assert_eq!(records[1].prev_lsn, begin_lsn);

    // Record 2: ABORT
    assert_eq!(records[2].record_type, RecordType::Abort);
    assert_eq!(records[2].lsn, abort_lsn);
    assert_eq!(records[2].prev_lsn, insert_lsn);
    assert_eq!(records[2].txt_id, txn_id);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_transaction_concurrent_begin_commit() {
    let dir = env::temp_dir().join("osirisdb_txn_test_concurrent");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let log_path = dir.join("wal.log");
    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let tm = Arc::new(TransactionManager::new(Arc::clone(&log_manager)));

    let num_threads = 10;
    let txns_per_thread = 5;

    let mut handles = Vec::new();
    for _ in 0..num_threads {
        let tm_clone = Arc::clone(&tm);
        handles.push(thread::spawn(move || {
            for _ in 0..txns_per_thread {
                let mut txn = tm_clone.begin().unwrap();
                assert_eq!(txn.status, TxnStatus::Active);
                assert!(txn.last_lsn > 0);

                // Commit transaction (participating in group commit)
                tm_clone.commit(&mut txn).unwrap();
                assert_eq!(txn.status, TxnStatus::Committed);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // All transactions have finished, ATT must be clean
    assert_eq!(tm.active_txn_count(), 0);

    // Flushed LSN should have covered all 50 BEGIN + 50 COMMIT records = 100 LSNs
    assert!(log_manager.get_flushed_lsn() >= (num_threads * txns_per_thread * 2) as u64);

    drop(tm);
    drop(log_manager);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_table_heap_delete_tuple_wal_record_and_chaining() {
    let dir = env::temp_dir().join("osirisdb_txn_test_delete");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let storage = Storage::new_or_create(&dir).unwrap();
    std::fs::create_dir_all(storage.schema_path("shop_db", "public")).unwrap();

    let log_path = dir.join("wal.log");
    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let tm = Arc::new(TransactionManager::new(Arc::clone(&log_manager)));

    let mut th = TableHeap::open_with_log_manager(
        &storage,
        "shop_db",
        "public",
        "items",
        Arc::clone(&log_manager),
    )
    .unwrap();

    let mut interner = Interner::new();
    let schema = vec![
        col(&mut interner, "item_id", DataType::Int, false),
        col(&mut interner, "name", DataType::VarChar(Some(30)), false),
    ];

    // 1. BEGIN txn
    let mut txn = tm.begin().unwrap();
    let txn_id = txn.txn_id;
    let begin_lsn = txn.last_lsn;

    // 2. INSERT tuple
    let name_sym = interner.intern("Keyboard");
    let row = vec![Value::Int(1), Value::String(name_sym)];
    let (page_id, slot_id) = th
        .insert_tuple(&schema, &row, &interner, Some(&mut txn))
        .unwrap();
    let insert_lsn = txn.last_lsn;
    assert!(insert_lsn > begin_lsn);

    let rid = RecordId { page_id, slot_id };

    // Verify tuple exists before delete
    let fetched = th.get_tuple(rid, &schema, &interner).unwrap();
    assert_eq!(fetched, Some(row));

    // 3. DELETE tuple
    let deleted = th.delete_tuple(rid, Some(&mut txn)).unwrap();
    assert!(deleted);
    let delete_lsn = txn.last_lsn;
    assert!(delete_lsn > insert_lsn);

    // Verify tuple is gone
    let fetched_after = th.get_tuple(rid, &schema, &interner).unwrap();
    assert_eq!(fetched_after, None);

    // 4. COMMIT txn
    tm.commit(&mut txn).unwrap();
    let commit_lsn = txn.last_lsn;
    assert!(commit_lsn > delete_lsn);

    drop(th);
    drop(tm);
    drop(log_manager);

    // 5. Read physical WAL file
    let records = read_all_log_records(&log_path);
    assert_eq!(records.len(), 4);

    // Record 0: BEGIN
    assert_eq!(records[0].record_type, RecordType::Begin);
    assert_eq!(records[0].lsn, begin_lsn);
    assert_eq!(records[0].prev_lsn, 0);

    // Record 1: INSERT
    assert_eq!(records[1].record_type, RecordType::Insert);
    assert_eq!(records[1].lsn, insert_lsn);
    assert_eq!(records[1].prev_lsn, begin_lsn);
    assert_eq!(records[1].txt_id, txn_id);
    assert_eq!(records[1].page_id, page_id);
    assert_eq!(records[1].offset, slot_id);

    // Record 2: DELETE (with before_image containing original tuple bytes, after_image empty)
    assert_eq!(records[2].record_type, RecordType::Delete);
    assert_eq!(records[2].lsn, delete_lsn);
    assert_eq!(records[2].prev_lsn, insert_lsn);
    assert_eq!(records[2].txt_id, txn_id);
    assert_eq!(records[2].page_id, page_id);
    assert_eq!(records[2].offset, slot_id);
    assert!(!records[2].before_image.is_empty());
    assert!(records[2].after_image.is_empty());

    // Record 3: COMMIT
    assert_eq!(records[3].record_type, RecordType::Commit);
    assert_eq!(records[3].lsn, commit_lsn);
    assert_eq!(records[3].prev_lsn, delete_lsn);
    assert_eq!(records[3].txt_id, txn_id);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_table_heap_update_tuple_wal_record_and_chaining() {
    let dir = env::temp_dir().join("osirisdb_txn_test_update");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let storage = Storage::new_or_create(&dir).unwrap();
    std::fs::create_dir_all(storage.schema_path("shop_db", "public")).unwrap();

    let log_path = dir.join("wal.log");
    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let tm = Arc::new(TransactionManager::new(Arc::clone(&log_manager)));

    let mut th = TableHeap::open_with_log_manager(
        &storage,
        "shop_db",
        "public",
        "products",
        Arc::clone(&log_manager),
    )
    .unwrap();

    let mut interner = Interner::new();
    let schema = vec![
        col(&mut interner, "id", DataType::Int, false),
        col(&mut interner, "title", DataType::VarChar(Some(50)), false),
    ];

    // 1. BEGIN txn
    let mut txn = tm.begin().unwrap();
    let txn_id = txn.txn_id;
    let begin_lsn = txn.last_lsn;

    // 2. INSERT original row: (10, "Original Title")
    let orig_title = interner.intern("Original Title");
    let orig_row = vec![Value::Int(10), Value::String(orig_title)];
    let (page_id, slot_id) = th
        .insert_tuple(&schema, &orig_row, &interner, Some(&mut txn))
        .unwrap();
    let insert_lsn = txn.last_lsn;
    assert!(insert_lsn > begin_lsn);

    let rid = RecordId { page_id, slot_id };

    // 3. UPDATE row to: (10, "Updated Title")
    let new_title = interner.intern("Updated Title");
    let new_row = vec![Value::Int(10), Value::String(new_title)];
    let updated = th
        .update_tuple(&schema, &new_row, &interner, rid, Some(&mut txn))
        .unwrap();
    assert!(updated);
    let update_lsn = txn.last_lsn;
    assert!(update_lsn > insert_lsn);

    // Verify row reflects the updated values
    let fetched = th.get_tuple(rid, &schema, &interner).unwrap();
    assert_eq!(fetched, Some(new_row));

    // 4. COMMIT txn
    tm.commit(&mut txn).unwrap();
    let commit_lsn = txn.last_lsn;
    assert!(commit_lsn > update_lsn);

    drop(th);
    drop(tm);
    drop(log_manager);

    // 5. Read physical WAL file
    let records = read_all_log_records(&log_path);
    assert_eq!(records.len(), 4);

    // Record 0: BEGIN
    assert_eq!(records[0].record_type, RecordType::Begin);
    assert_eq!(records[0].lsn, begin_lsn);

    // Record 1: INSERT
    assert_eq!(records[1].record_type, RecordType::Insert);
    assert_eq!(records[1].lsn, insert_lsn);
    assert_eq!(records[1].prev_lsn, begin_lsn);

    // Record 2: UPDATE (with both before_image and after_image present)
    assert_eq!(records[2].record_type, RecordType::Update);
    assert_eq!(records[2].lsn, update_lsn);
    assert_eq!(records[2].prev_lsn, insert_lsn);
    assert_eq!(records[2].txt_id, txn_id);
    assert_eq!(records[2].page_id, page_id);
    assert_eq!(records[2].offset, slot_id);
    assert!(!records[2].before_image.is_empty());
    assert!(!records[2].after_image.is_empty());
    assert_ne!(records[2].before_image, records[2].after_image);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_file_registry_mapping_and_table_heap_integration() {
    let dir = env::temp_dir().join("osirisdb_file_registry_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let storage = Storage::new_or_create(&dir).unwrap();
    std::fs::create_dir_all(storage.schema_path("shop_db", "public")).unwrap();
    let users_path = storage.table_path("shop_db", "public", "users").unwrap();
    let orders_path = storage.table_path("shop_db", "public", "orders").unwrap();

    // 1. Test FileRegistry mapping
    let registry = FileRegistry::new();
    let users_id = registry.register(&users_path);
    let orders_id = registry.register(&orders_path);

    assert_eq!(users_id, 1);
    assert_eq!(orders_id, 2);

    // Re-registering the same path returns the existing ID
    assert_eq!(registry.register(&users_path), users_id);
    assert_eq!(registry.get_id(&users_path), Some(users_id));
    assert_eq!(registry.get_path(users_id), Some(users_path.clone()));

    // 2. Open TableHeap with LogManager and set its file_id
    let log_path = dir.join("wal.log");
    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let tm = Arc::new(TransactionManager::new(Arc::clone(&log_manager)));

    let mut th = TableHeap::open_with_log_manager(
        &storage,
        "shop_db",
        "public",
        "users",
        Arc::clone(&log_manager),
    )
    .unwrap();
    th.set_file_id(users_id);
    assert_eq!(th.file_id(), users_id);

    let mut interner = Interner::new();
    let schema = vec![col(&mut interner, "id", DataType::Int, false)];

    let mut txn = tm.begin().unwrap();
    let row = vec![Value::Int(777)];
    th.insert_tuple(&schema, &row, &interner, Some(&mut txn))
        .unwrap();
    tm.commit(&mut txn).unwrap();

    drop(th);
    drop(tm);
    drop(log_manager);

    // 3. Inspect physical WAL records: verify record.file_id == users_id
    let records = read_all_log_records(&log_path);
    assert_eq!(records.len(), 3); // BEGIN, INSERT, COMMIT

    assert_eq!(records[0].record_type, RecordType::Begin);
    assert_eq!(records[0].file_id, 0); // Lifecycle record

    assert_eq!(records[1].record_type, RecordType::Insert);
    assert_eq!(records[1].file_id, users_id); // Data record stamped with registered file_id!

    assert_eq!(records[2].record_type, RecordType::Commit);
    assert_eq!(records[2].file_id, 0); // Lifecycle record

    let _ = std::fs::remove_dir_all(&dir);
}
