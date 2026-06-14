use osirisdb::ast::{CreateDatabaseStmt, CreateSchemaStmt};
use osirisdb::catalog::{CatalogError, CatalogManager};
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

fn schema_stmt(name: Option<Symbol>, if_not_exists: bool) -> CreateSchemaStmt {
    CreateSchemaStmt {
        name,
        authorization: None,
        if_not_exists,
    }
}

#[test]
fn test_create_schema_success() {
    let (mut m, s) = setup(&["mydb", "myschema", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[2]).unwrap();
    assert!(
        m.create_schema(s[0], schema_stmt(Some(s[1]), false), s[2])
            .is_ok()
    );
    assert!(m.schema_exists(s[0], s[1]));
}

#[test]
fn test_create_schema_database_not_found() {
    let (mut m, s) = setup(&["mydb", "myschema", "postgres"]);
    let err = m
        .create_schema(s[0], schema_stmt(Some(s[1]), false), s[2])
        .unwrap_err();
    assert_eq!(err, CatalogError::DatabaseNotFound(s[0]));
}

#[test]
fn test_create_schema_already_exists_error() {
    let (mut m, s) = setup(&["mydb", "myschema", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[2]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1]), false), s[2])
        .unwrap();
    let err = m
        .create_schema(s[0], schema_stmt(Some(s[1]), false), s[2])
        .unwrap_err();
    assert_eq!(err, CatalogError::SchemaAlreadyExists(s[1]));
}

#[test]
fn test_create_schema_if_not_exists_silent() {
    let (mut m, s) = setup(&["mydb", "myschema", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[2]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1]), false), s[2])
        .unwrap();
    assert!(
        m.create_schema(s[0], schema_stmt(Some(s[1]), true), s[2])
            .is_ok()
    );
}

#[test]
fn test_get_schema_success() {
    let (mut m, s) = setup(&["mydb", "myschema", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[2]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1]), false), s[2])
        .unwrap();
    let entry = m.get_schema(s[0], s[1]).unwrap();
    assert_eq!(entry.name, s[1]);
    assert_eq!(entry.database, s[0]);
}

#[test]
fn test_get_schema_not_found() {
    let (mut m, s) = setup(&["mydb", "myschema", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[2]).unwrap();
    let err = m.get_schema(s[0], s[1]).unwrap_err();
    assert_eq!(err, CatalogError::SchemaNotFound(s[1]));
}

#[test]
fn test_drop_schema_success() {
    let (mut m, s) = setup(&["mydb", "myschema", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[2]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1]), false), s[2])
        .unwrap();
    assert!(m.drop_schema(s[0], s[1], false).is_ok());
    assert!(!m.schema_exists(s[0], s[1]));
}

#[test]
fn test_drop_schema_if_exists_silent() {
    let (mut m, s) = setup(&["mydb", "myschema", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[2]).unwrap();
    assert!(m.drop_schema(s[0], s[1], true).is_ok());
}

#[test]
fn test_schema_oid_increments() {
    let (mut m, s) = setup(&["mydb", "s1", "s2", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[3]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1]), false), s[3])
        .unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[2]), false), s[3])
        .unwrap();
    let oid1 = m.get_schema(s[0], s[1]).unwrap().oid;
    let oid2 = m.get_schema(s[0], s[2]).unwrap().oid;
    assert!(oid2 > oid1);
}
