use std::{
    env::temp_dir,
    fs,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use osirisdb::{
    ast::{
        ColumnDef, CreateTableStmt, DataType, ForLockingClause, LockStrength, ObjectName, Value,
        WaitPolicy,
    },
    binder::{
        Binder,
        bound::{BoundSelectStmt, database::BoundCreateDatabaseStmt, insert::BoundInsertStmt},
    },
    catalog::{CatalogManager, objects::ColumnEntry},
    common::{Interner, symbol::Symbol},
    executor::{ExecutionError, ExecutionResult, Executor},
    storage::{
        BufferPool, FileRegistry, LogManager, RecordId, Storage, TableHeap, TransactionManager,
        TupleHeader, TupleInfoMask, is_tuple_visible,
        txn::lock::{lock_mode::RowLockMode, resource::LockResource},
    },
};

static TEST_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

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

fn setup_test_table(
    executor: &mut Executor,
    db: Symbol,
    schema: Symbol,
    table: Symbol,
    user: Symbol,
    col_defs: Vec<ColumnDef>,
) {
    executor
        .execute_create_database(BoundCreateDatabaseStmt {
            name: db,
            if_not_exists: true,
            owner: user,
            encoding: None,
            locale: None,
            tablespace: None,
            connection_limit: None,
        })
        .unwrap();

    let binder = Binder::new(&executor.catalog, user);
    let bound_create = binder
        .bind_create_table(
            db,
            schema,
            CreateTableStmt {
                if_not_exist: true,
                temporary: false,
                unlogged: false,
                name: ObjectName(vec![schema, table]),
                columns: col_defs,
                constraints: vec![],
                inherits: vec![],
                partitions: vec![],
                with_options: vec![],
                table_space: None,
                on_commit: None,
                as_query: None,
            },
        )
        .unwrap();
    executor.execute_create_table(bound_create).unwrap();
}

/// Scenario 1: Txn 1 locks row with FOR UPDATE. Txn 2 attempts update_tuple
/// on that row in a separate thread, blocking until Txn 1 commits.
#[test]
fn test_for_update_blocks_concurrent_update() {
    let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = temp_dir().join(format!("osirisdb_row_lock_test_1_{}", count));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let storage = Storage::new_or_create(&dir).unwrap();
    fs::create_dir_all(storage.schema_path("shop_db", "public")).unwrap();

    let log_path = dir.join("wal.log");
    let lm = Arc::new(LogManager::new(&log_path).unwrap());
    let file_registry = Arc::new(FileRegistry::open_or_create(&dir).unwrap());
    let buffer_pool = Arc::new(Mutex::new(BufferPool::new(64)));
    let tm = Arc::new(TransactionManager::new(
        Arc::clone(&lm),
        file_registry,
        Arc::clone(&buffer_pool),
    ));

    let mut th = TableHeap::open(&storage, "shop_db", "public", "items").unwrap();
    let mut interner = Interner::new();
    let schema = vec![
        col(&mut interner, "id", DataType::Int, false),
        col(&mut interner, "name", DataType::VarChar(Some(30)), false),
    ];

    let mut txn1 = tm.begin().unwrap();
    let row1 = vec![Value::Int(1), Value::String(interner.intern("Item 1"))];
    let (pid, sid) = th
        .insert_tuple(&schema, &row1, &interner, Some(&mut txn1), Some(&lm))
        .unwrap();
    let rid = RecordId {
        page_id: pid,
        slot_id: sid,
    };
    tm.commit(&mut txn1).unwrap();

    // 1. Txn 1 acquires exclusive lock on row 1
    let mut txn1 = tm.begin().unwrap();
    let resource = LockResource::Row {
        file_id: th.file_id(),
        rid,
    };
    tm.lock_manager()
        .acquire(&mut txn1, resource, RowLockMode::Update)
        .unwrap();

    // 2. Spawn Txn 2 in another thread trying to update row 1
    let update_done = Arc::new(AtomicBool::new(false));
    let update_done_clone = Arc::clone(&update_done);

    let tm_clone = Arc::clone(&tm);
    let lm_clone = Arc::clone(&lm);
    let dir_clone = dir.clone();

    let handle = thread::spawn(move || {
        let storage2 = Storage::new_or_create(&dir_clone).unwrap();
        let mut th2 = TableHeap::open(&storage2, "shop_db", "public", "items").unwrap();
        let mut interner2 = Interner::new();
        let schema2 = vec![
            col(&mut interner2, "id", DataType::Int, false),
            col(&mut interner2, "name", DataType::VarChar(Some(30)), false),
        ];
        let mut txn2 = tm_clone.begin().unwrap();
        let new_row = vec![
            Value::Int(1),
            Value::String(interner2.intern("Updated Item 1")),
        ];

        // Should block here until Txn 1 releases lock
        th2.update_tuple(
            &schema2,
            &new_row,
            &interner2,
            rid,
            Some(&mut txn2),
            Some(tm_clone.lock_manager().as_ref()),
            Some(&lm_clone),
        )
        .unwrap();

        update_done_clone.store(true, Ordering::SeqCst);
        tm_clone.commit(&mut txn2).unwrap();
    });

    // Verify Txn 2 is blocked while Txn 1 is active
    thread::sleep(Duration::from_millis(50));
    assert!(
        !update_done.load(Ordering::SeqCst),
        "Txn 2 should be blocked waiting for row lock"
    );

    // 3. Commit Txn 1, which releases lock and unblocks Txn 2
    tm.commit(&mut txn1).unwrap();

    handle.join().unwrap();
    assert!(
        update_done.load(Ordering::SeqCst),
        "Txn 2 should complete update after Txn 1 commits"
    );

    let _ = fs::remove_dir_all(&dir);
}

/// Scenario 2: Txn 1 locks row 1 with FOR UPDATE. Txn 2 runs SELECT ... FOR UPDATE NOWAIT.
/// Verify Txn 2 returns an immediate LockConflict error.
#[test]
fn test_for_update_nowait_fails_immediately() {
    let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = temp_dir().join(format!("osirisdb_row_lock_test_2_{}", count));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let interner = Interner::new();
    let db_sym = interner.intern("shop_db");
    let schema_sym = interner.intern("public");
    let table_sym = interner.intern("items");
    let user_sym = interner.intern("postgres");
    let col_id = interner.intern("id");
    let col_name = interner.intern("name");

    let col_defs = vec![
        ColumnDef {
            name: col_id,
            data_type: DataType::Int,
            collation: None,
            constraints: vec![],
            generated: None,
        },
        ColumnDef {
            name: col_name,
            data_type: DataType::VarChar(Some(30)),
            collation: None,
            constraints: vec![],
            generated: None,
        },
    ];

    let log_path = dir.join("wal.log");
    let lm = Arc::new(LogManager::new(&log_path).unwrap());
    let file_registry = Arc::new(FileRegistry::open_or_create(&dir).unwrap());
    let buffer_pool = Arc::new(Mutex::new(BufferPool::new(64)));
    let tm = Arc::new(TransactionManager::new(
        Arc::clone(&lm),
        file_registry,
        Arc::clone(&buffer_pool),
    ));

    let catalog = CatalogManager::new(interner.clone());
    let storage = Storage::new_or_create(&dir).unwrap();
    let mut executor = Executor::new(catalog, user_sym, storage);

    setup_test_table(
        &mut executor,
        db_sym,
        schema_sym,
        table_sym,
        user_sym,
        col_defs,
    );

    // Insert row 1 via executor
    let row1 = vec![Value::Int(1), Value::String(interner.intern("Item 1"))];
    executor
        .execute_insert_table(BoundInsertStmt {
            db: db_sym,
            schema: schema_sym,
            table: table_sym,
            rows: vec![row1],
        })
        .unwrap();

    let heap_handle = executor
        .get_table_heap(db_sym, schema_sym, table_sym)
        .unwrap();
    let table_entry = executor
        .catalog
        .get_table(db_sym, schema_sym, table_sym)
        .unwrap();
    let columns = table_entry.columns.clone();

    let records = heap_handle
        .lock()
        .unwrap()
        .scan_records(&columns, &interner)
        .unwrap();
    let rid = records[0].0;
    let file_id = heap_handle.lock().unwrap().file_id();

    // Txn 1 locks row 1
    let mut txn1 = tm.begin().unwrap();
    let resource = LockResource::Row { file_id, rid };
    tm.lock_manager()
        .acquire(&mut txn1, resource, RowLockMode::Update)
        .unwrap();

    // Txn 2 runs SELECT ... FOR UPDATE NOWAIT
    let mut txn2 = tm.begin().unwrap();
    let select_stmt = BoundSelectStmt {
        db: db_sym,
        schema: schema_sym,
        table: table_sym,
        columns,
        predicate: None,
        for_locking: Some(ForLockingClause {
            strength: LockStrength::Update,
            tables: vec![],
            wait_policy: WaitPolicy::NoWait,
        }),
    };

    let result = executor.execute_select_table_with_txn(
        select_stmt,
        Some(&mut txn2),
        Some(tm.lock_manager().as_ref()),
    );

    match result {
        Err(ExecutionError::Storage(msg)) => {
            assert!(
                msg.contains("NOWAIT"),
                "Expected NOWAIT conflict error, got: {}",
                msg
            );
        }
        other => panic!("Expected Storage error with NOWAIT, got: {:?}", other),
    }

    tm.commit(&mut txn1).unwrap();
    let _ = fs::remove_dir_all(&dir);
}

/// Scenario 3: Insert rows (1) and (2). Txn 1 locks row 1.
/// Txn 2 runs SELECT ... FOR UPDATE SKIP LOCKED.
/// Verify Txn 2 bypasses locked row 1 and returns only row 2.
#[test]
fn test_for_update_skip_locked_bypasses_locked_rows() {
    let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = temp_dir().join(format!("osirisdb_row_lock_test_3_{}", count));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let interner = Interner::new();
    let db_sym = interner.intern("shop_db");
    let schema_sym = interner.intern("public");
    let table_sym = interner.intern("items");
    let user_sym = interner.intern("postgres");
    let col_id = interner.intern("id");
    let col_name = interner.intern("name");

    let col_defs = vec![
        ColumnDef {
            name: col_id,
            data_type: DataType::Int,
            collation: None,
            constraints: vec![],
            generated: None,
        },
        ColumnDef {
            name: col_name,
            data_type: DataType::VarChar(Some(30)),
            collation: None,
            constraints: vec![],
            generated: None,
        },
    ];

    let log_path = dir.join("wal.log");
    let lm = Arc::new(LogManager::new(&log_path).unwrap());
    let file_registry = Arc::new(FileRegistry::open_or_create(&dir).unwrap());
    let buffer_pool = Arc::new(Mutex::new(BufferPool::new(64)));
    let tm = Arc::new(TransactionManager::new(
        Arc::clone(&lm),
        file_registry,
        Arc::clone(&buffer_pool),
    ));

    let catalog = CatalogManager::new(interner.clone());
    let storage = Storage::new_or_create(&dir).unwrap();
    let mut executor = Executor::new(catalog, user_sym, storage);

    setup_test_table(
        &mut executor,
        db_sym,
        schema_sym,
        table_sym,
        user_sym,
        col_defs,
    );

    // Insert row 1 and row 2
    let row1 = vec![Value::Int(1), Value::String(interner.intern("Item 1"))];
    let row2 = vec![Value::Int(2), Value::String(interner.intern("Item 2"))];
    executor
        .execute_insert_table(BoundInsertStmt {
            db: db_sym,
            schema: schema_sym,
            table: table_sym,
            rows: vec![row1, row2],
        })
        .unwrap();

    let heap_handle = executor
        .get_table_heap(db_sym, schema_sym, table_sym)
        .unwrap();
    let table_entry = executor
        .catalog
        .get_table(db_sym, schema_sym, table_sym)
        .unwrap();
    let columns = table_entry.columns.clone();

    let records = heap_handle
        .lock()
        .unwrap()
        .scan_records(&columns, &interner)
        .unwrap();
    let rid1 = records[0].0;
    let file_id = heap_handle.lock().unwrap().file_id();

    // Txn 1 locks row 1
    let mut txn1 = tm.begin().unwrap();
    let resource1 = LockResource::Row { file_id, rid: rid1 };
    tm.lock_manager()
        .acquire(&mut txn1, resource1, RowLockMode::Update)
        .unwrap();

    // Txn 2 runs SELECT * FROM items FOR UPDATE SKIP LOCKED
    let mut txn2 = tm.begin().unwrap();
    let select_stmt = BoundSelectStmt {
        db: db_sym,
        schema: schema_sym,
        table: table_sym,
        columns,
        predicate: None,
        for_locking: Some(ForLockingClause {
            strength: LockStrength::Update,
            tables: vec![],
            wait_policy: WaitPolicy::SkipLocked,
        }),
    };

    let result = executor
        .execute_select_table_with_txn(
            select_stmt,
            Some(&mut txn2),
            Some(tm.lock_manager().as_ref()),
        )
        .unwrap();

    match result {
        ExecutionResult::Selected { rows } => {
            assert_eq!(rows.len(), 1, "SKIP LOCKED should only return row 2");
            assert_eq!(rows[0][0], Value::Int(2));
        }
        other => panic!("Expected Selected result, got {:?}", other),
    }

    tm.commit(&mut txn1).unwrap();
    tm.commit(&mut txn2).unwrap();
    let _ = fs::remove_dir_all(&dir);
}

/// Scenario 4: Txn 1 stamps a row lock on a page tuple.
/// Verify physical tuple on page has header.xmax == txn_1.txn_id and
/// TupleInfoMask::XMAX_IS_LOCK_ONLY. Verify snapshot read still sees the row.
#[test]
fn test_xmax_is_lock_only_preserves_visibility() {
    let count = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = temp_dir().join(format!("osirisdb_row_lock_test_4_{}", count));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let storage = Storage::new_or_create(&dir).unwrap();
    fs::create_dir_all(storage.schema_path("shop_db", "public")).unwrap();

    let log_path = dir.join("wal.log");
    let lm = Arc::new(LogManager::new(&log_path).unwrap());
    let file_registry = Arc::new(FileRegistry::open_or_create(&dir).unwrap());
    let buffer_pool = Arc::new(Mutex::new(BufferPool::new(64)));
    let tm = Arc::new(TransactionManager::new(
        Arc::clone(&lm),
        file_registry,
        Arc::clone(&buffer_pool),
    ));

    let mut th = TableHeap::open(&storage, "shop_db", "public", "items").unwrap();
    let mut interner = Interner::new();
    let schema = vec![
        col(&mut interner, "id", DataType::Int, false),
        col(&mut interner, "name", DataType::VarChar(Some(30)), false),
    ];

    let mut txn_init = tm.begin().unwrap();
    let row1 = vec![Value::Int(100), Value::String(interner.intern("Product"))];
    let (pid, sid) = th
        .insert_tuple(&schema, &row1, &interner, Some(&mut txn_init), Some(&lm))
        .unwrap();
    let rid = RecordId {
        page_id: pid,
        slot_id: sid,
    };
    tm.commit(&mut txn_init).unwrap();

    // Txn 1 stamps row lock
    let mut txn1 = tm.begin().unwrap();
    th.stamp_row_lock(rid, txn1.txn_id).unwrap();

    // Verify raw page bytes
    let bp = th.buffer_pool_handle();
    let mut bp_lock = bp.lock().unwrap();
    let frame_id = bp_lock.pin_page(th.file_id(), rid.page_id).unwrap();
    let raw_bytes = bp_lock
        .get_page(frame_id)
        .get_tuple(rid.slot_id)
        .unwrap()
        .to_vec();
    bp_lock.unpin_page(frame_id, false);

    let mut header = TupleHeader::from_bytes(&raw_bytes[..28]).unwrap();
    assert_eq!(
        header.xmax, txn1.txn_id,
        "xmax must be stamped with locking txn_id"
    );
    assert!(
        TupleInfoMask::is_set(header.infomask, TupleInfoMask::XMAX_IS_LOCK_ONLY),
        "XMAX_IS_LOCK_ONLY flag must be set"
    );

    // Snapshot read in Txn 2 must still see row 1 as visible
    let txn2 = tm.begin().unwrap();
    let snap2 = tm.take_snapshot();
    assert!(
        is_tuple_visible(&mut header, &snap2, tm.clog(), txn2.txn_id),
        "Row locked with XMAX_IS_LOCK_ONLY must remain visible to concurrent snapshot reads"
    );

    tm.commit(&mut txn1).unwrap();
    let _ = fs::remove_dir_all(&dir);
}
