use osirisdb::ast::{ColumnConstraint, ColumnDef, CreateTableStmt, DataType, ObjectName, Value};
use osirisdb::binder::Binder;
use osirisdb::catalog::CatalogManager;
use osirisdb::common::interner::Interner;
use osirisdb::executor::Executor;
use osirisdb::storage::Storage;
use std::env;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn tmp(name: &str) -> PathBuf {
    env::temp_dir().join(format!("osirisdb_exec_insert_{}", name))
}

fn rm(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn test_insert_1000_rows_performance() {
    let path = tmp("perf_1000");
    rm(&path);

    let interner = Interner::new();
    let db_sym = interner.intern("mydb");
    let schema_sym = interner.intern("myschema");
    let table_sym = interner.intern("users");
    let col_id = interner.intern("id");
    let col_name = interner.intern("name");
    let user_sym = interner.intern("postgres");

    let catalog = CatalogManager::new(interner);
    let storage = Storage::new_or_create(&path).unwrap();
    let mut executor = Executor::new(catalog, user_sym, storage);

    // Create DB, Schema, and Table
    executor
        .execute_create_database(osirisdb::binder::bound::database::BoundCreateDatabaseStmt {
            name: db_sym,
            if_not_exists: true,
            owner: user_sym,
            encoding: None,
            locale: None,
            tablespace: None,
            connection_limit: None,
        })
        .unwrap();

    executor
        .execute_create_schema(
            db_sym,
            osirisdb::binder::bound::schema::BoundCreateSchemaStmt {
                name: Some(schema_sym),
                authorization: None,
                if_not_exists: true,
            },
        )
        .unwrap();

    let columns = vec![
        ColumnDef {
            name: col_id,
            data_type: DataType::Int,
            collation: None,
            constraints: vec![ColumnConstraint::PrimaryKey],
            generated: None,
        },
        ColumnDef {
            name: col_name,
            data_type: DataType::VarChar(Some(255)),
            collation: None,
            constraints: vec![],
            generated: None,
        },
    ];

    let binder = Binder::new(&executor.catalog, user_sym);
    let bound_create = binder
        .bind_create_table(
            db_sym,
            schema_sym,
            CreateTableStmt {
                if_not_exist: false,
                temporary: false,
                unlogged: false,
                name: ObjectName(vec![schema_sym, table_sym]),
                columns,
                constraints: vec![],
                inherits: vec![],
                partitions: vec![],
                with_options: vec![],
                table_space: None,
                on_commit: None,
                as_query: None,
            },
        )
        .unwrap();

    executor.execute_create_table(bound_create).unwrap();

    // Generate 1000 rows. A few will have duplicate ID (every 100th row is a duplicate of ID 1)
    let mut rows_to_insert = Vec::new();
    for i in 1..=1000 {
        let id = if i > 1 && i % 100 == 0 {
            1 // duplicate
        } else {
            i as i64
        };
        rows_to_insert.push(vec![Value::Int(id), Value::String(user_sym)]);
    }

    println!("Starting insertion of 1000 rows one-by-one...");
    let start_time = Instant::now();
    let mut success_count = 0;
    let mut duplicate_count = 0;

    for row in rows_to_insert {
        let bound_insert = osirisdb::binder::bound::insert::BoundInsertStmt {
            db: db_sym,
            schema: schema_sym,
            table: table_sym,
            rows: vec![row],
        };
        match executor.execute_insert_table(bound_insert) {
            Ok(_) => success_count += 1,
            Err(e) => {
                if e.to_string().contains("duplicate key value") {
                    duplicate_count += 1;
                } else {
                    panic!("Unexpected insertion error: {:?}", e);
                }
            }
        }
    }

    let elapsed = start_time.elapsed();
    println!(
        "\n=======================================================\n\
         Time taken to insert 1000 rows (one-by-one, with duplicates): {:?}\n\
         Success: {}, Duplicates rejected: {}\n\
         =======================================================\n",
        elapsed, success_count, duplicate_count
    );

    // Verify correct counts
    assert_eq!(success_count, 990);
    assert_eq!(duplicate_count, 10);

    rm(&path);
}
