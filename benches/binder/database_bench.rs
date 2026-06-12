use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use osirisdb::ast::CreateDatabaseStmt;
use osirisdb::binder::Binder;
use osirisdb::catalog::CatalogManager;
use osirisdb::common::interner::Interner;
use osirisdb::common::symbol::Symbol;

/// Builds a catalog with `n` existing databases and returns
/// the manager plus a fresh name symbol not yet in the catalog.
fn setup_with_n_databases(n: usize) -> (CatalogManager, Symbol, Symbol) {
    let mut interner = Interner::new();
    let session = interner.intern("postgres");
    let existing: Vec<Symbol> = (0..n)
        .map(|i| interner.intern(&format!("db{}", i)))
        .collect();
    let new_name = interner.intern("new_db");
    let mut m = CatalogManager::new(interner);
    for &name in &existing {
        m.create_database(
            CreateDatabaseStmt {
                name,
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
    }
    (m, new_name, session)
}

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

/// Benchmarks how fast the binder validates and resolves a
/// `CREATE DATABASE` statement against catalogs of varying sizes.
///
/// This measures catalog lookup cost as the number of databases grows.
fn bench_bind_create_database(c: &mut Criterion) {
    let mut group = c.benchmark_group("binder_create_database");

    for n in [0usize, 10, 100, 1000] {
        let (m, new_name, session) = setup_with_n_databases(n);
        let binder = Binder::new(&m, session);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("catalog_size", n),
            &new_name,
            |b, &name| b.iter(|| binder.bind_create_database(black_box(stmt(name, false)))),
        );
    }

    group.finish();
}

/// Benchmarks the `IF NOT EXISTS` path — database already exists,
/// binder should skip the error and return quickly.
fn bench_bind_if_not_exists(c: &mut Criterion) {
    let mut interner = Interner::new();
    let name = interner.intern("mydb");
    let session = interner.intern("postgres");
    let mut m = CatalogManager::new(interner);
    m.create_database(stmt(name, false), session).unwrap();

    let binder = Binder::new(&m, session);

    c.bench_function("binder_create_database_if_not_exists", |b| {
        b.iter(|| binder.bind_create_database(black_box(stmt(name, true))))
    });
}

/// Benchmarks binding a fully specified statement with all optional
/// fields set — encoding, locale, tablespace, owner, connection limit.
fn bench_bind_full_statement(c: &mut Criterion) {
    let mut interner = Interner::new();
    let name = interner.intern("mydb");
    let session = interner.intern("postgres");
    let alice = interner.intern("alice");
    let utf8 = interner.intern("UTF8");
    let locale = interner.intern("en_US");
    let ts = interner.intern("myspace");
    let m = CatalogManager::new(interner);
    let binder = Binder::new(&m, session);

    c.bench_function("binder_create_database_full", |b| {
        b.iter(|| {
            binder.bind_create_database(black_box(CreateDatabaseStmt {
                name,
                if_not_exists: false,
                owner: Some(alice),
                encoding: Some(utf8),
                locale: Some(locale),
                tablespace: Some(ts),
                connection_limit: Some(50),
            }))
        })
    });
}

criterion_group!(
    benches,
    bench_bind_create_database,
    bench_bind_if_not_exists,
    bench_bind_full_statement,
);
criterion_main!(benches);
