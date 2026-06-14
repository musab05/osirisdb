use criterion::{BenchmarkId, Criterion, Throughput};
use osirisdb::ast::{CreateDatabaseStmt, CreateSchemaStmt, CreateTableStmt, ObjectName};
use osirisdb::binder::Binder;
use osirisdb::catalog::CatalogManager;
use osirisdb::common::interner::Interner;
use osirisdb::common::symbol::Symbol;
use osirisdb::executor::Executor;

fn create_db_stmt(name: Symbol) -> CreateDatabaseStmt {
    CreateDatabaseStmt {
        name,
        if_not_exists: false,
        owner: None,
        encoding: None,
        locale: None,
        tablespace: None,
        connection_limit: None,
    }
}

fn schema_stmt(name: Symbol) -> CreateSchemaStmt {
    CreateSchemaStmt {
        name: Some(name),
        authorization: None,
        if_not_exists: false,
    }
}

fn table_stmt(name: Symbol, schema: Symbol, if_not_exist: bool) -> CreateTableStmt {
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

fn setup_with_n_tables(n: usize) -> (Executor, Symbol, Symbol, Symbol, Symbol) {
    let mut interner = Interner::new();
    let session = interner.intern("postgres");
    let db = interner.intern("mydb");
    let schema = interner.intern("myschema");
    let new_table = interner.intern("new_table");
    let mut catalog = CatalogManager::new(interner);

    catalog
        .create_database(create_db_stmt(db), session)
        .unwrap();
    catalog
        .create_schema(db, schema_stmt(schema), session)
        .unwrap();

    for i in 0..n {
        let name = catalog.interner.intern(&format!("table{}", i));
        catalog
            .create_table(db, schema, name, vec![], false)
            .unwrap();
    }

    let executor = Executor::new_in_memory(catalog, session);
    (executor, db, schema, new_table, session)
}

fn bench_bind_and_execute(c: &mut Criterion) {
    let mut group = c.benchmark_group("executor_create_table");

    for n in [0usize, 10, 100, 1000] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("catalog_size", n), &n, |b, &n| {
            b.iter_batched(
                || setup_with_n_tables(n),
                |(mut executor, db, schema, new_table, session)| {
                    let binder = Binder::new(&executor.catalog, session);
                    let bound = binder
                        .bind_create_table(
                            db,
                            schema,
                            std::hint::black_box(table_stmt(new_table, schema, false)),
                        )
                        .unwrap();
                    executor.execute_create_table(bound).unwrap()
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_execute_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("executor_create_table_execute_only");

    for n in [0usize, 10, 100, 1000] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("catalog_size", n), &n, |b, &n| {
            b.iter_batched(
                || {
                    let (executor, db, schema, new_table, session) = setup_with_n_tables(n);
                    let binder = Binder::new(&executor.catalog, session);
                    let bound = binder
                        .bind_create_table(db, schema, table_stmt(new_table, schema, false))
                        .unwrap();
                    (executor, bound)
                },
                |(mut executor, bound)| {
                    executor
                        .execute_create_table(std::hint::black_box(bound))
                        .unwrap()
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

pub fn bench(c: &mut Criterion) {
    bench_bind_and_execute(c);
    bench_execute_only(c);
}
