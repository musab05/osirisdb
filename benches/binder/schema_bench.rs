use criterion::{BenchmarkId, Criterion, Throughput, black_box};
use osirisdb::ast::{CreateDatabaseStmt, CreateSchemaStmt};
use osirisdb::binder::Binder;
use osirisdb::catalog::CatalogManager;
use osirisdb::common::interner::Interner;
use osirisdb::common::symbol::Symbol;

fn setup_with_n_schemas(n: usize) -> (CatalogManager, Symbol, Symbol, Symbol) {
    let mut interner = Interner::new();
    let session = interner.intern("postgres");
    let db = interner.intern("mydb");
    let new_schema = interner.intern("new_schema");

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

    for i in 0..n {
        let schema_name = m.interner.intern(&format!("schema{}", i));
        m.create_schema(
            db,
            CreateSchemaStmt {
                name: Some(schema_name),
                authorization: None,
                if_not_exists: false,
            },
            session,
        )
        .unwrap();
    }

    (m, db, new_schema, session)
}

fn stmt(name: Option<Symbol>, if_not_exists: bool) -> CreateSchemaStmt {
    CreateSchemaStmt {
        name,
        authorization: None,
        if_not_exists,
    }
}

pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("binder_create_schema");

    for n in [0usize, 10, 100, 1000] {
        let (m, db, new_schema, session) = setup_with_n_schemas(n);
        let binder = Binder::new(&m, session);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("catalog_size", n),
            &new_schema,
            |b, &name| {
                b.iter(|| {
                    let _ = binder.bind_create_schema(db, black_box(stmt(Some(name), false)));
                })
            },
        );
    }

    group.finish();
}
