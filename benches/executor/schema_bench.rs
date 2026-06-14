use criterion::{BenchmarkId, Criterion, Throughput};
use osirisdb::ast::{CreateDatabaseStmt, CreateSchemaStmt};
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

fn schema_stmt(name: Symbol, if_not_exists: bool) -> CreateSchemaStmt {
    CreateSchemaStmt {
        name: Some(name),
        authorization: None,
        if_not_exists,
    }
}

fn setup_with_n_schemas(n: usize) -> (Executor, Symbol, Symbol, Symbol) {
    let mut interner = Interner::new();
    let session = interner.intern("postgres");
    let db = interner.intern("mydb");
    let new_schema = interner.intern("new_schema");
    let mut catalog = CatalogManager::new(interner);

    catalog
        .create_database(create_db_stmt(db), session)
        .unwrap();

    for i in 0..n {
        let name = catalog.interner.intern(&format!("schema{}", i));
        catalog
            .create_schema(db, schema_stmt(name, false), session)
            .unwrap();
    }

    let executor = Executor::new_in_memory(catalog, session);
    (executor, db, new_schema, session)
}

fn bench_bind_and_execute(c: &mut Criterion) {
    let mut group = c.benchmark_group("executor_create_schema");

    for n in [0usize, 10, 100, 1000] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("catalog_size", n), &n, |b, &n| {
            b.iter_batched(
                || setup_with_n_schemas(n),
                |(mut executor, db, new_schema, session)| {
                    let binder = Binder::new(&executor.catalog, session);
                    let bound = binder
                        .bind_create_schema(
                            db,
                            std::hint::black_box(schema_stmt(new_schema, false)),
                        )
                        .unwrap();
                    executor.execute_create_schema(db, bound).unwrap()
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

fn bench_execute_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("executor_create_schema_execute_only");

    for n in [0usize, 10, 100, 1000] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("catalog_size", n), &n, |b, &n| {
            b.iter_batched(
                || {
                    let (executor, db, new_schema, session) = setup_with_n_schemas(n);
                    let binder = Binder::new(&executor.catalog, session);
                    let bound = binder
                        .bind_create_schema(db, schema_stmt(new_schema, false))
                        .unwrap();
                    (executor, db, bound)
                },
                |(mut executor, db, bound)| {
                    executor
                        .execute_create_schema(db, std::hint::black_box(bound))
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
