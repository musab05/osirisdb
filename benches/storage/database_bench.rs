use criterion::{Criterion, criterion_group, criterion_main};
use osirisdb::{
    ast::{DataType, Value},
    catalog::objects::ColumnEntry,
    common::Interner,
    storage::{Storage, TableHeap, log::log_manager::LogManager},
};
use std::{hint::black_box, sync::Arc};

fn bench_create_database_dir(c: &mut Criterion) {
    let tmp = std::env::temp_dir().join("osirisdb_bench_storage");
    // clean up any leftover from previous runs before starting
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let storage = Storage::new(&tmp).unwrap();

    let mut counter = 0u64;
    c.bench_function("storage_create_database_dir", |b| {
        b.iter(|| {
            let name = format!("bench_db_{}", counter);
            counter += 1;
            storage.create_database_dir(black_box(&name)).unwrap();
        })
    });

    std::fs::remove_dir_all(&tmp).unwrap();
}

fn bench_drop_database_dir(c: &mut Criterion) {
    let tmp = std::env::temp_dir().join("osirisdb_bench_storage_drop");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let storage = Storage::new(&tmp).unwrap();

    let mut counter = 0u64;
    c.bench_function("storage_drop_database_dir", |b| {
        b.iter_batched(
            || {
                let name = format!("bench_db_{}", counter);
                counter += 1;
                storage.create_database_dir(&name).unwrap();
                name
            },
            |name| storage.drop_database_dir(black_box(&name)).unwrap(),
            criterion::BatchSize::SmallInput,
        )
    });

    std::fs::remove_dir_all(&tmp).unwrap();
}

fn bench_table_heap_insert(c: &mut Criterion) {
    let tmp = std::env::temp_dir().join("osirisdb_bench_th_insert");
    let _ = std::fs::remove_dir_all(&tmp);
    let storage = Storage::new_or_create(&tmp).unwrap();
    std::fs::create_dir_all(storage.schema_path("bench_db", "public")).unwrap();

    let mut interner = Interner::new();
    let id_sym = interner.intern("id");
    let name_sym = interner.intern("name");
    let val_sym = interner.intern("Alice");

    let schema = vec![
        ColumnEntry {
            name: id_sym,
            data_type: DataType::Int,
            nullable: false,
            default: None,
            is_unique: false,
            is_primary_key: false,
        },
        ColumnEntry {
            name: name_sym,
            data_type: DataType::VarChar(Some(50)),
            nullable: false,
            default: None,
            is_unique: false,
            is_primary_key: false,
        },
    ];

    let row = vec![Value::Int(42), Value::String(val_sym)];

    // Benchmark 1: Insert without WAL
    let mut th_no_wal = TableHeap::open(&storage, "bench_db", "public", "table_no_wal").unwrap();
    c.bench_function("table_heap_insert_without_wal", |b| {
        b.iter(|| {
            th_no_wal
                .insert_tuple(black_box(&schema), black_box(&row), &interner, None, None)
                .unwrap();
        })
    });

    // Benchmark 2: Insert with WAL
    let log_path = tmp.join("bench_wal.log");
    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let mut th_wal = TableHeap::open(&storage, "bench_db", "public", "table_wal").unwrap();
    c.bench_function("table_heap_insert_with_wal", |b| {
        b.iter(|| {
            th_wal
                .insert_tuple(
                    black_box(&schema),
                    black_box(&row),
                    &interner,
                    None,
                    Some(&log_manager),
                )
                .unwrap();
        })
    });

    std::fs::remove_dir_all(&tmp).unwrap();
}

fn bench_checkpoint_manager(c: &mut Criterion) {
    use osirisdb::storage::{CheckpointManager, TransactionManager};

    let tmp = std::env::temp_dir().join("osirisdb_bench_checkpoint");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let log_path = tmp.join("bench_ckpt.log");
    let meta_path = tmp.join("checkpoint.meta");
    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let tm = Arc::new(TransactionManager::new(Arc::clone(&log_manager)));
    let ckpt_mgr = CheckpointManager::new(Arc::clone(&log_manager), Arc::clone(&tm), &meta_path);

    // Keep some transactions active in ATT
    let _txn1 = tm.begin().unwrap();
    let _txn2 = tm.begin().unwrap();

    c.bench_function("checkpoint_manager_execute", |b| {
        b.iter(|| {
            black_box(ckpt_mgr.checkpoint().unwrap());
        })
    });

    drop(tm);
    drop(log_manager);
    let _ = std::fs::remove_dir_all(&tmp);
}

fn bench_aries_recovery(c: &mut Criterion) {
    use osirisdb::storage::{FileRegistry, RecoveryEngine, TransactionManager};

    let tmp = std::env::temp_dir().join("osirisdb_bench_recovery");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let storage = Storage::new_or_create(&tmp).unwrap();
    std::fs::create_dir_all(storage.schema_path("bench_db", "public")).unwrap();
    let table_path = storage.table_path("bench_db", "public", "users").unwrap();

    let registry = Arc::new(FileRegistry::new());
    let file_id = registry.register(&table_path);

    let log_path = tmp.join("wal.log");
    let meta_path = tmp.join("checkpoint.meta");
    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let tm = Arc::new(TransactionManager::new(Arc::clone(&log_manager)));

    let mut interner = Interner::new();
    let id_sym = interner.intern("id");
    let name_sym = interner.intern("name");
    let val_sym = interner.intern("RecoverMe");

    let schema = vec![
        ColumnEntry {
            name: id_sym,
            data_type: DataType::Int,
            nullable: false,
            default: None,
            is_unique: false,
            is_primary_key: false,
        },
        ColumnEntry {
            name: name_sym,
            data_type: DataType::VarChar(Some(50)),
            nullable: false,
            default: None,
            is_unique: false,
            is_primary_key: false,
        },
    ];

    let row = vec![Value::Int(1), Value::String(val_sym)];

    let mut th = TableHeap::open(&storage, "bench_db", "public", "users").unwrap();
    th.set_file_id(file_id);

    // Populate WAL with 100 logged transactions
    for _ in 0..50 {
        let mut txn = tm.begin().unwrap();
        th.insert_tuple(&schema, &row, &interner, Some(&mut txn), Some(&log_manager))
            .unwrap();
        tm.commit(&mut txn).unwrap();
    }
    // And 1 uncommitted loser transaction
    let mut loser = tm.begin().unwrap();
    th.insert_tuple(
        &schema,
        &row,
        &interner,
        Some(&mut loser),
        Some(&log_manager),
    )
    .unwrap();

    log_manager.flush().unwrap();
    drop(th);
    drop(tm);
    drop(log_manager);

    // Benchmark full ARIES Recovery Engine (Analysis + Redo + Undo)
    c.bench_function("aries_recovery_engine_recover", |b| {
        b.iter(|| {
            let log_mgr = Arc::new(LogManager::new(&log_path).unwrap());
            let engine = RecoveryEngine::new(&log_path, &meta_path, Arc::clone(&registry), log_mgr);
            black_box(engine.recover().unwrap());
        })
    });

    let _ = std::fs::remove_dir_all(&tmp);
}

criterion_group!(
    benches,
    bench_create_database_dir,
    bench_drop_database_dir,
    bench_table_heap_insert,
    bench_checkpoint_manager,
    bench_aries_recovery
);
criterion_main!(benches);
