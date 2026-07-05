use criterion::{BenchmarkId, Criterion, Throughput, black_box};
use osirisdb::ast::{CreateDatabaseStmt, CreateSchemaStmt};
use osirisdb::catalog::CatalogManager;
use osirisdb::common::interner::Interner;
use osirisdb::common::symbol::Symbol;
use osirisdb::storage::Storage;
use std::path::PathBuf;

struct TempDir {
    path: PathBuf,
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

static BENCH_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn unique_temp_dir() -> TempDir {
    let id = BENCH_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!("osirisdb_bench_cat_table_{}", id));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    TempDir { path }
}

fn make_manager(n: usize) -> (CatalogManager, Symbol, Symbol, Vec<Symbol>, TempDir) {
    let temp = unique_temp_dir();
    let mut interner = Interner::new();
    let session = interner.intern("postgres");
    let db = interner.intern("mydb");
    let schema = interner.intern("myschema");
    let names: Vec<Symbol> = (0..n)
        .map(|i| interner.intern(&format!("table{}", i)))
        .collect();
    let mut m = CatalogManager::new(interner);

    let storage = Storage::new_or_create(&temp.path).unwrap();
    storage.create_database_dir("mydb").unwrap();

    m.create_database(
        CreateDatabaseStmt {
            name: db,
            if_not_exists: false,
            owner: None,
            encoding: None,
            locale: None,
            tablespace: None,
            connection_limit: None,
        },
        session,
    )
    .unwrap();

    storage.create_schema_dir("mydb", "myschema").unwrap();

    m.create_schema(
        db,
        CreateSchemaStmt {
            name: Some(schema),
            authorization: None,
            if_not_exists: false,
        },
        session,
    )
    .unwrap();

    for &name in &names {
        let name_str = m.interner.resolve(name);
        storage
            .create_table_file("mydb", "myschema", name_str)
            .unwrap();
        m.create_table(&storage, db, schema, name, vec![], vec![], false)
            .unwrap();
    }
    (m, db, schema, names, temp)
}

pub fn bench(c: &mut Criterion) {
    c.bench_function("catalog_table_create", |b| {
        b.iter_batched(
            || {
                let temp = unique_temp_dir();
                let storage = Storage::new_or_create(&temp.path).unwrap();
                storage.create_database_dir("mydb").unwrap();
                storage.create_schema_dir("mydb", "myschema").unwrap();

                let mut interner = Interner::new();
                let db = interner.intern("mydb");
                let schema = interner.intern("myschema");
                let table = interner.intern("mytable");
                let session = interner.intern("postgres");
                let mut m = CatalogManager::new(interner);
                m.create_database(
                    CreateDatabaseStmt {
                        name: db,
                        if_not_exists: false,
                        owner: None,
                        encoding: None,
                        locale: None,
                        tablespace: None,
                        connection_limit: None,
                    },
                    session,
                )
                .unwrap();
                m.create_schema(
                    db,
                    CreateSchemaStmt {
                        name: Some(schema),
                        authorization: None,
                        if_not_exists: false,
                    },
                    session,
                )
                .unwrap();
                (m, db, schema, table, storage, temp)
            },
            |(mut m, db, schema, table, storage, _temp)| {
                storage
                    .create_table_file("mydb", "myschema", m.interner.resolve(table))
                    .unwrap();
                m.create_table(
                    &storage,
                    db,
                    schema,
                    black_box(table),
                    vec![],
                    vec![],
                    false,
                )
                .unwrap();
            },
            criterion::BatchSize::SmallInput,
        )
    });

    let mut group = c.benchmark_group("catalog_table_exists");
    for n in [10usize, 100, 1000] {
        let (m, db, schema, names, _temp) = make_manager(n);
        let target = names[n / 2];
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("exists", n), &target, |b, &name| {
            b.iter(|| m.table_exists(db, schema, black_box(name)))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("catalog_table_get");
    for n in [10usize, 100, 1000] {
        let (m, db, schema, names, _temp) = make_manager(n);
        let target = names[n / 2];
        group.bench_with_input(BenchmarkId::new("get", n), &target, |b, &name| {
            b.iter(|| {
                let _ = m.get_table(db, schema, black_box(name));
            })
        });
    }
    group.finish();
}
