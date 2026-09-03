use std::{env, fs::File, io::Read, sync::Arc};

use osirisdb::{
    ast::{DataType, Value},
    catalog::objects::ColumnEntry,
    common::Interner,
    storage::{
        CheckpointData, CheckpointManager, FileRegistry, LogManager, RecordId, RecoveryEngine,
        Storage, TableHeap, TransactionManager,
        log::log_record::{LogRecord, RecordType},
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

/// Helper to read all [`LogRecord`]s from a physical WAL file.
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
fn test_checkpoint_data_serde_round_trip() {
    let data = CheckpointData {
        active_txns: vec![(1, 100), (2, 250), (3, 310)],
        dirty_pages: vec![((1, 0), 100), ((1, 1), 150), ((2, 0), 250)],
    };

    let bytes = data.serialize();
    let deserialized =
        CheckpointData::deserialize(&bytes).expect("Failed to deserialize CheckpointData");

    assert_eq!(data, deserialized);
}

#[test]
fn test_checkpoint_manager_writes_begin_end_and_meta() {
    let dir = env::temp_dir().join("osirisdb_ckpt_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let log_path = dir.join("wal.log");
    let meta_path = dir.join("checkpoint.meta");
    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let tm = Arc::new(TransactionManager::new(Arc::clone(&log_manager)));

    // Begin 2 transactions: txn 1 and txn 2
    let mut txn1 = tm.begin().unwrap();
    let txn2 = tm.begin().unwrap();

    // Commit txn 1. Txn 2 remains active in the ATT.
    tm.commit(&mut txn1).unwrap();

    // Run Checkpoint
    let ckpt_mgr = CheckpointManager::new(Arc::clone(&log_manager), Arc::clone(&tm), &meta_path);
    let ckpt_begin_lsn = ckpt_mgr.checkpoint().unwrap();

    // Verify checkpoint.meta exists on disk and contains begin_lsn
    assert!(meta_path.exists());
    let meta_bytes = std::fs::read(&meta_path).unwrap();
    assert_eq!(meta_bytes.len(), 8);
    let stored_lsn = u64::from_le_bytes(meta_bytes.try_into().unwrap());
    assert_eq!(stored_lsn, ckpt_begin_lsn);

    drop(tm);
    drop(log_manager);

    // Read physical WAL records
    let records = read_all_log_records(&log_path);
    let ckpt_begin_rec = records
        .iter()
        .find(|r| r.record_type == RecordType::CheckpointBegin)
        .unwrap();
    let ckpt_end_rec = records
        .iter()
        .find(|r| r.record_type == RecordType::CheckpointEnd)
        .unwrap();

    assert_eq!(ckpt_begin_rec.lsn, ckpt_begin_lsn);

    // Deserialize after_image from CheckpointEnd
    let ckpt_data = CheckpointData::deserialize(&ckpt_end_rec.after_image).unwrap();
    // Only txn 2 should be in the active transactions snapshot!
    assert_eq!(ckpt_data.active_txns.len(), 1);
    assert_eq!(ckpt_data.active_txns[0].0, txn2.txn_id);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_recovery_redo_committed_transaction_when_disk_is_blank() {
    let dir = env::temp_dir().join("osirisdb_recovery_redo_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let storage = Storage::new_or_create(&dir).unwrap();
    std::fs::create_dir_all(storage.schema_path("shop_db", "public")).unwrap();
    let table_path = storage.table_path("shop_db", "public", "users").unwrap();

    let registry = Arc::new(FileRegistry::new());
    let file_id = registry.register(&table_path);

    let log_path = dir.join("wal.log");
    let meta_path = dir.join("checkpoint.meta");
    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let tm = Arc::new(TransactionManager::new(Arc::clone(&log_manager)));

    let mut interner = Interner::new();
    let schema = vec![
        col(&mut interner, "id", DataType::Int, false),
        col(&mut interner, "name", DataType::VarChar(Some(50)), false),
    ];

    // 1. Run a transaction and commit
    let mut th = TableHeap::open(&storage, "shop_db", "public", "users").unwrap();
    th.set_file_id(file_id);

    let mut txn = tm.begin().unwrap();
    let name1 = interner.intern("Alice");
    let row1 = vec![Value::Int(1), Value::String(name1)];
    th.insert_tuple(
        &schema,
        &row1,
        &interner,
        Some(&mut txn),
        Some(&log_manager),
    )
    .unwrap();

    let name2 = interner.intern("Bob");
    let row2 = vec![Value::Int(2), Value::String(name2)];
    th.insert_tuple(
        &schema,
        &row2,
        &interner,
        Some(&mut txn),
        Some(&log_manager),
    )
    .unwrap();

    tm.commit(&mut txn).unwrap();

    // 2. Simulate CRASH before dirty pages are flushed:
    // Drop memory handles, overwrite table file with an empty 0-byte file
    drop(th);
    drop(tm);
    drop(log_manager);
    std::fs::write(&table_path, b"").unwrap();

    // 3. Restart & Run ARIES Recovery
    let log_manager_restart = Arc::new(LogManager::new(&log_path).unwrap());
    let engine = RecoveryEngine::new(
        &log_path,
        &meta_path,
        Arc::clone(&registry),
        log_manager_restart,
    );
    engine.recover().unwrap();

    // 4. Verify data was fully restored on disk by Redo phase!
    let mut th_recovered = TableHeap::open(&storage, "shop_db", "public", "users").unwrap();
    let rows = th_recovered.scan(&schema, &interner).unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], row1);
    assert_eq!(rows[1], row2);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_recovery_undo_uncommitted_loser_transaction() {
    let dir = env::temp_dir().join("osirisdb_recovery_undo_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let storage = Storage::new_or_create(&dir).unwrap();
    std::fs::create_dir_all(storage.schema_path("shop_db", "public")).unwrap();
    let table_path = storage.table_path("shop_db", "public", "products").unwrap();

    let registry = Arc::new(FileRegistry::new());
    let file_id = registry.register(&table_path);

    let log_path = dir.join("wal.log");
    let meta_path = dir.join("checkpoint.meta");
    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let tm = Arc::new(TransactionManager::new(Arc::clone(&log_manager)));

    let mut interner = Interner::new();
    let schema = vec![
        col(&mut interner, "id", DataType::Int, false),
        col(&mut interner, "title", DataType::VarChar(Some(50)), false),
    ];

    let mut th = TableHeap::open(&storage, "shop_db", "public", "products").unwrap();
    th.set_file_id(file_id);

    // 1. Transaction 1 (Committed): Inserts Item 1
    let mut txn1 = tm.begin().unwrap();
    let title1 = interner.intern("Phone");
    let row1 = vec![Value::Int(1), Value::String(title1)];
    let (p1, s1) = th
        .insert_tuple(
            &schema,
            &row1,
            &interner,
            Some(&mut txn1),
            Some(&log_manager),
        )
        .unwrap();
    let rid1 = RecordId {
        page_id: p1,
        slot_id: s1,
    };
    tm.commit(&mut txn1).unwrap();

    // 2. Transaction 2 (Loser / Uncommitted):
    // Inserts Item 2 and Updates Item 1, but NEVER commits (simulating a crash mid-flight)
    let mut txn2 = tm.begin().unwrap();
    let title2 = interner.intern("Laptop");
    let row2 = vec![Value::Int(2), Value::String(title2)];
    th.insert_tuple(
        &schema,
        &row2,
        &interner,
        Some(&mut txn2),
        Some(&log_manager),
    )
    .unwrap();

    let title1_updated = interner.intern("Phone Pro Max");
    let row1_updated = vec![Value::Int(1), Value::String(title1_updated)];
    th.update_tuple(
        &schema,
        &row1_updated,
        &interner,
        rid1,
        Some(&mut txn2),
        Some(&log_manager),
    )
    .unwrap();

    // Flush WAL to simulate log records reaching disk before sudden power loss
    log_manager.flush().unwrap();
    drop(th);
    drop(tm);
    drop(log_manager);

    // 3. Restart & Run ARIES Recovery
    let log_manager_restart = Arc::new(LogManager::new(&log_path).unwrap());
    let engine = RecoveryEngine::new(
        &log_path,
        &meta_path,
        Arc::clone(&registry),
        log_manager_restart,
    );
    engine.recover().unwrap();

    // 4. Verify on-disk rows:
    // - Item 1 must be restored to "Phone" (update undone!)
    // - Item 2 must be deleted (insert undone!)
    let mut th_recovered = TableHeap::open(&storage, "shop_db", "public", "products").unwrap();
    let rows = th_recovered.scan(&schema, &interner).unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0], row1); // Exactly Item 1 ("Phone")

    // 5. Verify physical WAL contains CLR and Abort records for Txn 2
    let records = read_all_log_records(&log_path);
    assert!(
        records
            .iter()
            .any(|r| r.record_type == RecordType::Compensation && r.txt_id == txn2.txn_id)
    );
    assert!(
        records
            .iter()
            .any(|r| r.record_type == RecordType::Abort && r.txt_id == txn2.txn_id)
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_recovery_with_checkpoint_redo_and_undo() {
    let dir = env::temp_dir().join("osirisdb_recovery_full_aries_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let storage = Storage::new_or_create(&dir).unwrap();
    std::fs::create_dir_all(storage.schema_path("shop_db", "public")).unwrap();
    let table_path = storage
        .table_path("shop_db", "public", "inventory")
        .unwrap();

    let registry = Arc::new(FileRegistry::new());
    let file_id = registry.register(&table_path);

    let log_path = dir.join("wal.log");
    let meta_path = dir.join("checkpoint.meta");
    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let tm = Arc::new(TransactionManager::new(Arc::clone(&log_manager)));

    let mut interner = Interner::new();
    let schema = vec![
        col(&mut interner, "id", DataType::Int, false),
        col(&mut interner, "tag", DataType::VarChar(Some(30)), false),
    ];

    let mut th = TableHeap::open(&storage, "shop_db", "public", "inventory").unwrap();
    th.set_file_id(file_id);

    // 1. Txn 1: Inserts Row 1 and commits
    let mut txn1 = tm.begin().unwrap();
    let tag1 = interner.intern("Alpha");
    let row1 = vec![Value::Int(1), Value::String(tag1)];
    th.insert_tuple(
        &schema,
        &row1,
        &interner,
        Some(&mut txn1),
        Some(&log_manager),
    )
    .unwrap();
    tm.commit(&mut txn1).unwrap();

    // 2. Run Fuzzy Checkpoint
    let ckpt_mgr = CheckpointManager::new(Arc::clone(&log_manager), Arc::clone(&tm), &meta_path);
    ckpt_mgr.checkpoint().unwrap();

    // 3. Txn 2 (Committed): Inserts Row 2 after checkpoint
    let mut txn2 = tm.begin().unwrap();
    let tag2 = interner.intern("Beta");
    let row2 = vec![Value::Int(2), Value::String(tag2)];
    th.insert_tuple(
        &schema,
        &row2,
        &interner,
        Some(&mut txn2),
        Some(&log_manager),
    )
    .unwrap();
    tm.commit(&mut txn2).unwrap();

    // 4. Txn 3 (Loser): Inserts Row 3 and crashes without committing
    let mut txn3 = tm.begin().unwrap();
    let tag3 = interner.intern("Gamma");
    let row3 = vec![Value::Int(3), Value::String(tag3)];
    th.insert_tuple(
        &schema,
        &row3,
        &interner,
        Some(&mut txn3),
        Some(&log_manager),
    )
    .unwrap();

    // Crash simulation
    log_manager.flush().unwrap();
    drop(th);
    drop(tm);
    drop(log_manager);

    // 5. Restart & Run ARIES Recovery
    let log_manager_restart = Arc::new(LogManager::new(&log_path).unwrap());
    let engine = RecoveryEngine::new(
        &log_path,
        &meta_path,
        Arc::clone(&registry),
        log_manager_restart,
    );
    engine.recover().unwrap();

    // 6. Verify final database state:
    // - Row 1 ("Alpha") present (committed before checkpoint)
    // - Row 2 ("Beta") present (committed after checkpoint)
    // - Row 3 ("Gamma") rolled back (Txn 3 loser)
    let mut th_recovered = TableHeap::open(&storage, "shop_db", "public", "inventory").unwrap();
    let rows = th_recovered.scan(&schema, &interner).unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], row1);
    assert_eq!(rows[1], row2);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_storage_clean_shutdown_and_restart_skips_recovery() {
    let dir = env::temp_dir().join("osirisdb_clean_shutdown_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let log_path = dir.join("wal.log");
    let meta_path = dir.join("checkpoint.meta");

    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let tm = Arc::new(TransactionManager::new(Arc::clone(&log_manager)));
    let ckpt_mgr = Arc::new(CheckpointManager::new(
        Arc::clone(&log_manager),
        Arc::clone(&tm),
        &meta_path,
    ));

    let mut storage = Storage::with_log_manager(&dir, Arc::clone(&log_manager)).unwrap();
    storage.with_checkpoint_manager(Arc::clone(&ckpt_mgr));

    std::fs::create_dir_all(storage.schema_path("shop_db", "public")).unwrap();
    let table_path = storage.table_path("shop_db", "public", "items").unwrap();
    let file_id = storage.file_registry().register(&table_path);

    let mut interner = Interner::new();
    let schema = vec![
        col(&mut interner, "id", DataType::Int, false),
        col(&mut interner, "name", DataType::VarChar(Some(30)), false),
    ];

    // Insert row and commit
    let mut th = TableHeap::open(&storage, "shop_db", "public", "items").unwrap();
    th.set_file_id(file_id);

    let mut txn = tm.begin().unwrap();
    let name = interner.intern("Keyboard");
    let row = vec![Value::Int(1), Value::String(name)];
    th.insert_tuple(&schema, &row, &interner, Some(&mut txn), Some(&log_manager))
        .unwrap();
    tm.commit(&mut txn).unwrap();

    // Perform clean shutdown
    storage.shutdown().unwrap();

    // Verify marker and meta file exist
    assert!(storage.has_clean_shutdown_marker());
    assert!(meta_path.exists());

    drop(th);
    drop(storage);
    drop(ckpt_mgr);
    drop(tm);
    drop(log_manager);

    // On restart: initialize new Storage and run recover_if_needed
    let log_manager_restart = Arc::new(LogManager::new(&log_path).unwrap());
    let storage_restart =
        Storage::with_log_manager(&dir, Arc::clone(&log_manager_restart)).unwrap();

    // recover_if_needed should return Ok(false) (recovery skipped because of clean shutdown marker)
    let recovered = storage_restart
        .recover_if_needed(&log_path, &meta_path)
        .unwrap();
    assert!(!recovered, "Clean shutdown should bypass ARIES recovery");

    // The marker should have been consumed / removed for future crash detection
    assert!(!storage_restart.has_clean_shutdown_marker());

    // Verify data is intact
    let mut th_reopened = TableHeap::open(&storage_restart, "shop_db", "public", "items").unwrap();
    let rows = th_reopened.scan(&schema, &interner).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0], row);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_storage_crash_triggers_recovery_on_startup() {
    let dir = env::temp_dir().join("osirisdb_crash_recovery_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let log_path = dir.join("wal.log");
    let meta_path = dir.join("checkpoint.meta");

    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let tm = Arc::new(TransactionManager::new(Arc::clone(&log_manager)));

    let storage = Storage::with_log_manager(&dir, Arc::clone(&log_manager)).unwrap();
    std::fs::create_dir_all(storage.schema_path("shop_db", "public")).unwrap();
    let table_path = storage.table_path("shop_db", "public", "orders").unwrap();
    let file_id = storage.file_registry().register(&table_path);

    let mut interner = Interner::new();
    let schema = vec![
        col(&mut interner, "id", DataType::Int, false),
        col(&mut interner, "item", DataType::VarChar(Some(30)), false),
    ];

    // Txn 1 committed
    let mut th = TableHeap::open(&storage, "shop_db", "public", "orders").unwrap();
    th.set_file_id(file_id);

    let mut txn1 = tm.begin().unwrap();
    let item1 = interner.intern("Monitor");
    let row1 = vec![Value::Int(10), Value::String(item1)];
    th.insert_tuple(
        &schema,
        &row1,
        &interner,
        Some(&mut txn1),
        Some(&log_manager),
    )
    .unwrap();
    tm.commit(&mut txn1).unwrap();

    // Flush WAL but crash WITHOUT calling storage.shutdown() (so no clean shutdown marker)
    log_manager.flush().unwrap();
    drop(th);
    drop(storage);
    drop(tm);
    drop(log_manager);

    // Overwrite table file to 0 bytes to simulate unwritten dirty pages on crash
    std::fs::write(&table_path, b"").unwrap();

    // Restart: No clean shutdown marker exists!
    let log_manager_restart = Arc::new(LogManager::new(&log_path).unwrap());
    let storage_restart =
        Storage::with_log_manager(&dir, Arc::clone(&log_manager_restart)).unwrap();

    assert!(!storage_restart.has_clean_shutdown_marker());

    // recover_if_needed runs ARIES recovery and returns Ok(true)
    let recovered = storage_restart
        .recover_if_needed(&log_path, &meta_path)
        .unwrap();
    assert!(recovered, "Unclean crash must execute ARIES recovery");

    // Verify committed data was restored by recovery Redo phase
    let mut th_recovered =
        TableHeap::open(&storage_restart, "shop_db", "public", "orders").unwrap();
    let rows = th_recovered.scan(&schema, &interner).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0], row1);

    let _ = std::fs::remove_dir_all(&dir);
}
