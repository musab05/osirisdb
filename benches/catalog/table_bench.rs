use criterion::{BenchmarkId, Criterion, Throughput, black_box};
use osirisdb::ast::{CreateDatabaseStmt, CreateSchemaStmt};
use osirisdb::catalog::CatalogManager;
use osirisdb::common::interner::Interner;
use osirisdb::common::symbol::Symbol;

fn make_manager(n: usize) -> (CatalogManager, Symbol, Symbol, Vec<Symbol>) {
    let mut interner = Interner::new();
    let session = interner.intern("postgres");
    let db = interner.intern("mydb");
    let schema = interner.intern("myschema");
    let names: Vec<Symbol> = (0..n)
        .map(|i| interner.intern(&format!("table{}", i)))
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
        m.create_table(db, schema, name, vec![], vec![], false)
            .unwrap();
    }
    (m, db, schema, names)
}

pub fn bench(c: &mut Criterion) {
    c.bench_function("catalog_table_create", |b| {
        b.iter_batched(
            || {
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
                (m, db, schema, table)
            },
            |(mut m, db, schema, table)| {
                m.create_table(db, schema, black_box(table), vec![], vec![], false)
                    .unwrap();
            },
            criterion::BatchSize::SmallInput,
        )
    });

    let mut group = c.benchmark_group("catalog_table_exists");
    for n in [10usize, 100, 1000] {
        let (m, db, schema, names) = make_manager(n);
        let target = names[n / 2];
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("exists", n), &target, |b, &name| {
            b.iter(|| m.table_exists(db, schema, black_box(name)))
        });
    }
    group.finish();

    let mut group = c.benchmark_group("catalog_table_get");
    for n in [10usize, 100, 1000] {
        let (m, db, schema, names) = make_manager(n);
        let target = names[n / 2];
        group.bench_with_input(BenchmarkId::new("get", n), &target, |b, &name| {
            b.iter(|| {
                let _ = m.get_table(db, schema, black_box(name));
            })
        });
    }
    group.finish();
}
