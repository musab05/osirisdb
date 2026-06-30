use criterion::{Criterion, Throughput};
use osirisdb::ast::{ColumnConstraint, ColumnDef, CreateTableStmt, DataType, ObjectName, Value};
use osirisdb::binder::Binder;
use osirisdb::catalog::CatalogManager;
use osirisdb::common::interner::Interner;
use osirisdb::common::symbol::Symbol;
use osirisdb::executor::Executor;
use osirisdb::storage::Storage;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static BENCH_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempDir {
    path: PathBuf,
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn unique_temp_dir() -> TempDir {
    let id = BENCH_COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!("osirisdb_bench_insert_{}", id));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    TempDir { path }
}

fn setup_executor_with_table() -> (Executor, Symbol, Symbol, Symbol, Symbol, TempDir) {
    let temp = unique_temp_dir();
    let interner = Interner::new();
    let db_sym = interner.intern("mydb");
    let schema_sym = interner.intern("myschema");
    let table_sym = interner.intern("users");
    let col_id = interner.intern("id");
    let col_name = interner.intern("name");
    let user_sym = interner.intern("postgres");

    let catalog = CatalogManager::new(interner);
    let storage = Storage::new_or_create(&temp.path).unwrap();
    let mut executor = Executor::new(catalog, user_sym, storage);

    // Create DB
    executor
        .execute_create_database(osirisdb::binder::bound::database::BoundCreateDatabaseStmt {
            name: db_sym,
            if_not_exists: true,
            owner: user_sym,
            encoding: None,
            locale: None,
            tablespace: None,
            connection_limit: None,
        })
        .unwrap();

    // Create Schema
    executor
        .execute_create_schema(
            db_sym,
            osirisdb::binder::bound::schema::BoundCreateSchemaStmt {
                name: Some(schema_sym),
                authorization: None,
                if_not_exists: true,
            },
        )
        .unwrap();

    // Create Table
    let columns = vec![
        ColumnDef {
            name: col_id,
            data_type: DataType::Int,
            collation: None,
            constraints: vec![ColumnConstraint::PrimaryKey],
            generated: None,
        },
        ColumnDef {
            name: col_name,
            data_type: DataType::VarChar(Some(255)),
            collation: None,
            constraints: vec![],
            generated: None,
        },
    ];

    let binder = Binder::new(&executor.catalog, user_sym);
    let bound_create = binder
        .bind_create_table(
            db_sym,
            schema_sym,
            CreateTableStmt {
                if_not_exist: false,
                temporary: false,
                unlogged: false,
                name: ObjectName(vec![schema_sym, table_sym]),
                columns,
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

    (executor, db_sym, schema_sym, table_sym, user_sym, temp)
}

fn bench_insert_one_by_one(c: &mut Criterion) {
    let mut group = c.benchmark_group("executor_insert_one_by_one");
    group.sample_size(10);
    group.throughput(Throughput::Elements(1000));

    // Generate rows
    let interner = Interner::new();
    let user_sym = interner.intern("postgres");
    let mut rows_to_insert = Vec::new();
    for i in 1..=1000 {
        let id = if i > 1 && i % 100 == 0 {
            1 // duplicate
        } else {
            i as i64
        };
        rows_to_insert.push(vec![Value::Int(id), Value::String(user_sym)]);
    }

    group.bench_function("insert_1000_rows_one_by_one", |b| {
        b.iter_batched(
            || (setup_executor_with_table(), rows_to_insert.clone()),
            |((mut executor, db_sym, schema_sym, table_sym, _, _temp), rows)| {
                for row in rows {
                    let bound_insert = osirisdb::binder::bound::insert::BoundInsertStmt {
                        db: db_sym,
                        schema: schema_sym,
                        table: table_sym,
                        rows: vec![row],
                    };
                    let _ = executor.execute_insert_table(bound_insert);
                }
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

fn bench_insert_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("executor_insert_batch");
    group.sample_size(10);
    group.throughput(Throughput::Elements(1000));

    // Generate rows with no duplicates (since duplicates abort the batch)
    let interner = Interner::new();
    let user_sym = interner.intern("postgres");
    let mut rows_to_insert = Vec::new();
    for i in 1..=1000 {
        rows_to_insert.push(vec![Value::Int(i as i64), Value::String(user_sym)]);
    }

    group.bench_function("insert_1000_rows_batch", |b| {
        b.iter_batched(
            || (setup_executor_with_table(), rows_to_insert.clone()),
            |((mut executor, db_sym, schema_sym, table_sym, _, _temp), rows)| {
                let bound_insert = osirisdb::binder::bound::insert::BoundInsertStmt {
                    db: db_sym,
                    schema: schema_sym,
                    table: table_sym,
                    rows,
                };
                executor.execute_insert_table(bound_insert).unwrap();
            },
            criterion::BatchSize::SmallInput,
        )
    });

    group.finish();
}

pub fn bench(c: &mut Criterion) {
    bench_insert_one_by_one(c);
    bench_insert_batch(c);
}
