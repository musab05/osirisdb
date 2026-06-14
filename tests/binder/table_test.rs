use osirisdb::ast::{
    ColumnDef, CreateDatabaseStmt, CreateSchemaStmt, CreateTableStmt, DataType, ObjectName,
};
use osirisdb::binder::{BindError, Binder};
use osirisdb::catalog::CatalogManager;
use osirisdb::common::interner::Interner;
use osirisdb::common::symbol::Symbol;

fn setup(names: &[&str]) -> (CatalogManager, Vec<Symbol>) {
    let mut interner = Interner::new();
    let symbols = names.iter().map(|n| interner.intern(n)).collect();
    (CatalogManager::new(interner), symbols)
}

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

fn schema_stmt(name: Option<Symbol>) -> CreateSchemaStmt {
    CreateSchemaStmt {
        name,
        authorization: None,
        if_not_exists: false,
    }
}

fn table_stmt(name: ObjectName, columns: Vec<ColumnDef>, if_not_exist: bool) -> CreateTableStmt {
    CreateTableStmt {
        if_not_exist,
        temporary: false,
        unlogged: false,
        name,
        columns,
        constraints: vec![],
        inherits: vec![],
        partitions: vec![],
        with_options: vec![],
        table_space: None,
        on_commit: None,
        as_query: None,
    }
}

fn column_def(name: Symbol, data_type: DataType) -> ColumnDef {
    ColumnDef {
        name,
        data_type,
        collation: None,
        constraints: vec![],
        generated: None,
    }
}

#[test]
fn test_bind_table_success() {
    let (mut m, s) = setup(&["mydb", "myschema", "mytable", "col1", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[4]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1])), s[4])
        .unwrap();

    let binder = Binder::new(&m, s[4]);
    let columns = vec![column_def(s[3], DataType::Int)];
    let obj_name = ObjectName(vec![s[1], s[2]]);
    let bound = binder
        .bind_create_table(s[0], s[1], table_stmt(obj_name, columns, false))
        .unwrap();

    assert_eq!(bound.db, s[0]);
    assert_eq!(bound.schema, s[1]);
    assert_eq!(bound.name, s[2]);
    assert_eq!(bound.columns.len(), 1);
    assert_eq!(bound.columns[0].name, s[3]);
    assert_eq!(bound.columns[0].data_type, DataType::Int);
    assert!(!bound.if_not_exists);
}

#[test]
fn test_bind_table_schema_not_found() {
    let (mut m, s) = setup(&["mydb", "myschema", "mytable", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[3]).unwrap();

    let binder = Binder::new(&m, s[3]);
    let obj_name = ObjectName(vec![s[1], s[2]]);
    let err = binder
        .bind_create_table(s[0], s[1], table_stmt(obj_name, vec![], false))
        .unwrap_err();
    assert_eq!(err, BindError::SchemaNotFound(s[1]));
}

#[test]
fn test_bind_table_already_exists() {
    let (mut m, s) = setup(&["mydb", "myschema", "mytable", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[3]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1])), s[3])
        .unwrap();

    // Create the table in catalog first
    m.create_table(s[0], s[1], s[2], vec![], false).unwrap();

    let binder = Binder::new(&m, s[3]);
    let obj_name = ObjectName(vec![s[1], s[2]]);
    let err = binder
        .bind_create_table(s[0], s[1], table_stmt(obj_name.clone(), vec![], false))
        .unwrap_err();
    assert_eq!(err, BindError::TableAlreadyExists(s[2]));

    // with IF NOT EXISTS should succeed
    let bound = binder
        .bind_create_table(s[0], s[1], table_stmt(obj_name, vec![], true))
        .unwrap();
    assert!(bound.if_not_exists);
}
