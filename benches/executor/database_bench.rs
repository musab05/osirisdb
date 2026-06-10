use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use rust_sql::ast::CreateDatabaseStmt;
use rust_sql::binder::Binder;
use rust_sql::catalog::CatalogManager;
use rust_sql::common::interner::Interner;
use rust_sql::common::symbol::Symbol;
use rust_sql::executor::Executor;

fn stmt(name: Symbol, if_not_exists: bool) -> CreateDatabaseStmt {
    CreateDatabaseStmt {
        name,
        if_not_exists,
        owner: None,
        encoding: None,
        locale: None,
        tablespace: None,
        connection_limit: None,
    }
}

/// Builds an executor with `n` pre-existing databases.
/// Returns the executor, a fresh unused name symbol, and the session symbol.
fn setup_with_n_databases(n: usize) -> (Executor, Symbol, Symbol) {
    let mut interner = Interner::new();
    let session = interner.intern("postgres");
    let existing: Vec<Symbol> = (0..n)
        .map(|i| interner.intern(&format!("db{}", i)))
        .collect();
    let new_name = interner.intern("new_db");
    let mut catalog = CatalogManager::new(interner);
    for &name in &existing {
        catalog.create_database(stmt(name, false), session).unwrap();
    }
    let executor = Executor::new_in_memory(catalog, session);
    (executor, new_name, session)
}

/// Benchmarks the full bind + execute path for `CREATE DATABASE`
/// against catalogs of varying sizes.
///
/// Measures how catalog size affects the combined binder lookup
/// and executor write cost.
fn bench_bind_and_execute(c: &mut Criterion) {
    let mut group = c.benchmark_group("executor_create_database");

    for n in [0usize, 10, 100, 1000] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("catalog_size", n), &n, |b, &n| {
            b.iter_batched(
                || setup_with_n_databases(n),
                |(mut executor, new_name, session)| {
                    let binder = Binder::new(&executor.catalog, session);
                    let bound = binder
                        .bind_create_database(std::hint::black_box(stmt(new_name, false)))
                        .unwrap();
                    executor.execute_create_database(bound).unwrap()
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

/// Benchmarks the execute-only path — binder already done,
/// measures raw catalog write cost.
fn bench_execute_only(c: &mut Criterion) {
    let mut group = c.benchmark_group("executor_create_database_execute_only");

    for n in [0usize, 10, 100, 1000] {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("catalog_size", n), &n, |b, &n| {
            b.iter_batched(
                || {
                    let (executor, new_name, session) = setup_with_n_databases(n);
                    let binder = Binder::new(&executor.catalog, session);
                    let bound = binder.bind_create_database(stmt(new_name, false)).unwrap();
                    (executor, bound)
                },
                |(mut executor, bound)| {
                    executor
                        .execute_create_database(std::hint::black_box(bound))
                        .unwrap()
                },
                criterion::BatchSize::SmallInput,
            )
        });
    }

    group.finish();
}

/// Benchmarks the IF NOT EXISTS path where the database already exists.
/// The executor should return quickly without a catalog write.
fn bench_execute_if_not_exists_existing(c: &mut Criterion) {
    c.bench_function("executor_create_database_if_not_exists", |b| {
        b.iter_batched(
            || {
                let mut interner = Interner::new();
                let name = interner.intern("mydb");
                let session = interner.intern("postgres");
                let mut catalog = CatalogManager::new(interner);
                catalog.create_database(stmt(name, false), session).unwrap();
                let executor = Executor::new_in_memory(catalog, session);
                let binder = Binder::new(&executor.catalog, session);
                let bound = binder.bind_create_database(stmt(name, true)).unwrap();
                (executor, bound)
            },
            |(mut executor, bound)| {
                executor
                    .execute_create_database(std::hint::black_box(bound))
                    .unwrap()
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

/// Benchmarks a fully specified statement with all optional fields set.
fn bench_execute_full_statement(c: &mut Criterion) {
    c.bench_function("executor_create_database_full", |b| {
        b.iter_batched(
            || {
                let mut interner = Interner::new();
                let name = interner.intern("mydb");
                let session = interner.intern("postgres");
                let alice = interner.intern("alice");
                let utf8 = interner.intern("UTF8");
                let locale = interner.intern("en_US");
                let ts = interner.intern("myspace");
                let catalog = CatalogManager::new(interner);
                let executor = Executor::new_in_memory(catalog, session);
                let binder = Binder::new(&executor.catalog, session);
                let bound = binder
                    .bind_create_database(CreateDatabaseStmt {
                        name,
                        if_not_exists: false,
                        owner: Some(alice),
                        encoding: Some(utf8),
                        locale: Some(locale),
                        tablespace: Some(ts),
                        connection_limit: Some(50),
                    })
                    .unwrap();
                (executor, bound)
            },
            |(mut executor, bound)| {
                executor
                    .execute_create_database(std::hint::black_box(bound))
                    .unwrap()
            },
            criterion::BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    benches,
    bench_bind_and_execute,
    bench_execute_only,
    bench_execute_if_not_exists_existing,
    bench_execute_full_statement,
);
criterion_main!(benches);
