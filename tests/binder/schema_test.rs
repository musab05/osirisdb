use osirisdb::ast::{CreateDatabaseStmt, CreateSchemaStmt};
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

fn schema_stmt(
    name: Option<Symbol>,
    authorization: Option<Symbol>,
    if_not_exists: bool,
) -> CreateSchemaStmt {
    CreateSchemaStmt {
        name,
        authorization,
        if_not_exists,
    }
}

#[test]
fn test_bind_schema_success() {
    let (mut m, s) = setup(&["mydb", "myschema", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[2]).unwrap();

    let binder = Binder::new(&m, s[2]);
    let bound = binder
        .bind_create_schema(s[0], schema_stmt(Some(s[1]), None, false))
        .unwrap();
    assert_eq!(bound.name, Some(s[1]));
    assert_eq!(bound.authorization, None);
    assert!(!bound.if_not_exists);
}

#[test]
fn test_bind_schema_database_not_found() {
    let (m, s) = setup(&["mydb", "myschema", "postgres"]);
    let binder = Binder::new(&m, s[2]);
    let err = binder
        .bind_create_schema(s[0], schema_stmt(Some(s[1]), None, false))
        .unwrap_err();
    assert_eq!(err, BindError::DatabaseNotFound(s[0]));
}

#[test]
fn test_bind_schema_already_exists() {
    let (mut m, s) = setup(&["mydb", "myschema", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[2]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1]), None, false), s[2])
        .unwrap();

    let binder = Binder::new(&m, s[2]);
    let err = binder
        .bind_create_schema(s[0], schema_stmt(Some(s[1]), None, false))
        .unwrap_err();
    assert_eq!(err, BindError::SchemaAlreadyExists(s[1]));
}

#[test]
fn test_bind_schema_if_not_exists() {
    let (mut m, s) = setup(&["mydb", "myschema", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[2]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1]), None, false), s[2])
        .unwrap();

    let binder = Binder::new(&m, s[2]);
    let bound = binder
        .bind_create_schema(s[0], schema_stmt(Some(s[1]), None, true))
        .unwrap();
    assert_eq!(bound.name, Some(s[1]));
    assert!(bound.if_not_exists);
}

#[test]
fn test_bind_schema_default_name() {
    let (mut m, s) = setup(&["mydb", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[1]).unwrap();

    let binder = Binder::new(&m, s[1]);
    let bound = binder
        .bind_create_schema(s[0], schema_stmt(None, None, false))
        .unwrap();
    assert_eq!(bound.name, Some(s[1]));
}
