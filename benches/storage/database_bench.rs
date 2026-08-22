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
                .insert_tuple(black_box(&schema), black_box(&row), &interner, None)
                .unwrap();
        })
    });

    // Benchmark 2: Insert with WAL
    let log_path = tmp.join("bench_wal.log");
    let log_manager = Arc::new(LogManager::new(&log_path).unwrap());
    let mut th_wal =
        TableHeap::open_with_log_manager(&storage, "bench_db", "public", "table_wal", log_manager)
            .unwrap();
    c.bench_function("table_heap_insert_with_wal", |b| {
        b.iter(|| {
            th_wal
                .insert_tuple(black_box(&schema), black_box(&row), &interner, None)
                .unwrap();
        })
    });

    std::fs::remove_dir_all(&tmp).unwrap();
}

criterion_group!(
    benches,
    bench_create_database_dir,
    bench_drop_database_dir,
    bench_table_heap_insert
);
criterion_main!(benches);
