use osirisdb::ast::{
    ColumnConstraint, ColumnDef, CreateDatabaseStmt, CreateSchemaStmt, CreateTableStmt, DataType,
    ObjectName, Value,
};
use osirisdb::binder::Binder;
use osirisdb::catalog::CatalogManager;
use osirisdb::common::interner::Interner;
use osirisdb::common::symbol::Symbol;
use osirisdb::executor::{ExecutionResult, Executor};
use osirisdb::storage::Storage;
use std::env;
use std::path::{Path, PathBuf};

fn tmp(name: &str) -> PathBuf {
    env::temp_dir().join(format!("osirisdb_exec_constraint_{}", name))
}

fn rm(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn test_primary_key_uniqueness() {
    let path = tmp("pk_uniq");
    rm(&path);

    let mut interner = Interner::new();
    let db_sym = interner.intern("mydb");
    let schema_sym = interner.intern("myschema");
    let table_sym = interner.intern("users");
    let col_id = interner.intern("id");
    let col_name = interner.intern("name");
    let user_sym = interner.intern("postgres");

    let catalog = CatalogManager::new(interner);
    let storage = Storage::new_or_create(&path).unwrap();
    let mut executor = Executor::new(catalog, user_sym, storage);

    // 1. Create DB, Schema, and Table with PRIMARY KEY
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

    // 2. Insert first row (ID: 1)
    let bound_insert1 = osirisdb::binder::bound::insert::BoundInsertStmt {
        db: db_sym,
        schema: schema_sym,
        table: table_sym,
        rows: vec![vec![Value::Int(1), Value::String(user_sym)]],
    };
    executor.execute_insert_table(bound_insert1).unwrap();

    // 3. Attempt to insert duplicate row (ID: 1) - should fail
    let bound_insert2 = osirisdb::binder::bound::insert::BoundInsertStmt {
        db: db_sym,
        schema: schema_sym,
        table: table_sym,
        rows: vec![vec![Value::Int(1), Value::String(user_sym)]],
    };
    let err = executor.execute_insert_table(bound_insert2).unwrap_err();
    assert!(err.to_string().contains("duplicate key value"));

    rm(&path);
}
