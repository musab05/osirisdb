use criterion::{BenchmarkId, Criterion, Throughput, black_box};
use osirisdb::ast::{CreateDatabaseStmt, CreateSchemaStmt, CreateTableStmt, ObjectName};
use osirisdb::binder::Binder;
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
    let path = std::env::temp_dir().join(format!("osirisdb_bench_bind_table_{}", id));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    TempDir { path }
}

fn setup_with_n_tables(n: usize) -> (CatalogManager, Symbol, Symbol, Symbol, Symbol, TempDir) {
    let temp = unique_temp_dir();
    let mut interner = Interner::new();
    let session = interner.intern("postgres");
    let db = interner.intern("mydb");
    let schema = interner.intern("myschema");
    let new_table = interner.intern("new_table");

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

    for i in 0..n {
        let table_name = m.interner.intern(&format!("table{}", i));
        let name_str = format!("table{}", i);
        storage
            .create_table_file("mydb", "myschema", &name_str)
            .unwrap();
        m.create_table(&storage, db, schema, table_name, vec![], vec![], false)
            .unwrap();
    }

    (m, db, schema, new_table, session, temp)
}

fn stmt(name: Symbol, schema: Symbol, if_not_exist: bool) -> CreateTableStmt {
    CreateTableStmt {
        if_not_exist,
        temporary: false,
        unlogged: false,
        name: ObjectName(vec![schema, name]),
        columns: vec![],
        constraints: vec![],
        inherits: vec![],
        partitions: vec![],
        with_options: vec![],
        table_space: None,
        on_commit: None,
        as_query: None,
    }
}

pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("binder_create_table");

    for n in [0usize, 10, 100, 1000] {
        let (m, db, schema, new_table, session, _temp) = setup_with_n_tables(n);
        let binder = Binder::new(&m, session);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("catalog_size", n),
            &new_table,
            |b, &name| {
                b.iter(|| {
                    let _ =
                        binder.bind_create_table(db, schema, black_box(stmt(name, schema, false)));
                })
            },
        );
    }

    group.finish();
}
