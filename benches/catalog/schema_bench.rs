use criterion::{BenchmarkId, Criterion, Throughput, black_box};
use osirisdb::ast::{CreateDatabaseStmt, CreateSchemaStmt};
use osirisdb::catalog::CatalogManager;
use osirisdb::common::interner::Interner;
use osirisdb::common::symbol::Symbol;

fn make_manager(n: usize) -> (CatalogManager, Symbol, Vec<Symbol>) {
    let mut interner = Interner::new();
    let session = interner.intern("postgres");
    let db = interner.intern("mydb");
    let names: Vec<Symbol> = (0..n)
        .map(|i| interner.intern(&format!("schema{}", i)))
        .collect();
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

    for &name in &names {
        m.create_schema(
            db,
            CreateSchemaStmt {
                name: Some(name),
                authorization: None,
                if_not_exists: false,
            },
            session,
        )
        .unwrap();
    }
    (m, db, names)
}

pub fn bench(c: &mut Criterion) {
    c.bench_function("catalog_schema_create", |b| {
        b.iter_batched(
            || {
                let mut interner = Interner::new();
                let db = interner.intern("mydb");
                let schema = interner.intern("myschema");
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
                (m, db, schema, session)
            },
            |(mut m, db, schema, session)| {
                m.create_schema(
                    db,
                    black_box(CreateSchemaStmt {
                        name: Some(schema),
                        authorization: None,
                        if_not_exists: false,
                    }),
                    session,
                )
                .unwrap();
            },
            criterion::BatchSize::SmallInput,
        )
    });

    let mut group = c.benchmark_group("catalog_schema_exists");
    for n in [10usize, 100, 1000] {
        let (m, db, names) = make_manager(n);
        let target = names[n / 2];
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("exists", n), &target, |b, &name| {
            b.iter(|| m.schema_exists(db, black_box(name)))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("catalog_schema_get");
    for n in [10usize, 100, 1000] {
        let (m, db, names) = make_manager(n);
        let target = names[n / 2];
        group.bench_with_input(BenchmarkId::new("get", n), &target, |b, &name| {
            b.iter(|| {
                let _ = m.get_schema(db, black_box(name));
            })
        });
    }
    group.finish();
}
