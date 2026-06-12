use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use osirisdb::ast::CreateDatabaseStmt;
use osirisdb::catalog::CatalogManager;
use osirisdb::common::interner::Interner;
use osirisdb::common::symbol::Symbol;

fn make_manager(n: usize) -> (CatalogManager, Vec<Symbol>) {
    let mut interner = Interner::new();
    let session = interner.intern("postgres");
    let names: Vec<Symbol> = (0..n)
        .map(|i| interner.intern(&format!("db{}", i)))
        .collect();
    let mut m = CatalogManager::new(interner);
    for &name in &names {
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
    (m, names)
}

pub fn bench(c: &mut Criterion) {
    // create
    c.bench_function("catalog_database_create", |b| {
        b.iter_batched(
            || {
                let mut interner = Interner::new();
                let name = interner.intern("mydb");
                let session = interner.intern("postgres");
                (CatalogManager::new(interner), name, session)
            },
            |(mut m, name, session)| {
                m.create_database(
                    black_box(CreateDatabaseStmt {
                        name,
                        if_not_exists: false,
                        owner: None,
                        encoding: None,
                        locale: None,
                        tablespace: None,
                        connection_limit: None,
                    }),
                    session,
                )
                .unwrap();
            },
            criterion::BatchSize::SmallInput,
        )
    });

    // exists lookup at various catalog sizes
    let mut group = c.benchmark_group("catalog_database_exists");
    for n in [10usize, 100, 1000] {
        let (m, names) = make_manager(n);
        let target = names[n / 2];
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("exists", n), &target, |b, &name| {
            b.iter(|| m.database_exists(black_box(name)))
        });
    }
    group.finish();

    // get lookup at various catalog sizes
    let mut group = c.benchmark_group("catalog_database_get");
    for n in [10usize, 100, 1000] {
        let (m, names) = make_manager(n);
        let target = names[n / 2];
        group.bench_with_input(BenchmarkId::new("get", n), &target, |b, &name| {
            b.iter(|| m.get_database(black_box(name)))
        });
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
