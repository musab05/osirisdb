use rust_sql::ast::CreateDatabaseStmt;
use rust_sql::catalog::{CatalogError, CatalogManager};
use rust_sql::common::interner::Interner;
use rust_sql::common::symbol::Symbol;

fn setup(names: &[&str]) -> (CatalogManager, Vec<Symbol>) {
    let mut interner = Interner::new();
    let symbols = names.iter().map(|n| interner.intern(n)).collect();
    (CatalogManager::new(interner), symbols)
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

// ── create_database ───────────────────────────────────────────────────────────

#[test]
fn test_create_success() {
    let (mut m, s) = setup(&["mydb", "postgres"]);
    assert!(m.create_database(stmt(s[0], false), s[1]).is_ok());
    assert!(m.database_exists(s[0]));
}

#[test]
fn test_create_already_exists_error() {
    let (mut m, s) = setup(&["mydb", "postgres"]);
    m.create_database(stmt(s[0], false), s[1]).unwrap();
    let err = m.create_database(stmt(s[0], false), s[1]).unwrap_err();
    assert_eq!(err, CatalogError::DatabaseAlreadyExists(s[0]));
}

#[test]
fn test_create_if_not_exists_silent() {
    let (mut m, s) = setup(&["mydb", "postgres"]);
    m.create_database(stmt(s[0], false), s[1]).unwrap();
    assert!(m.create_database(stmt(s[0], true), s[1]).is_ok());
}

#[test]
fn test_create_defaults_owner_to_session_user() {
    let (mut m, s) = setup(&["mydb", "postgres"]);
    m.create_database(stmt(s[0], false), s[1]).unwrap();
    assert_eq!(m.get_database(s[0]).unwrap().owner, s[1]);
}

#[test]
fn test_create_explicit_owner() {
    let (mut m, s) = setup(&["mydb", "postgres", "alice"]);
    let explicit = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: false,
        owner: Some(s[2]),
        encoding: None,
        locale: None,
        tablespace: None,
        connection_limit: None,
    };
    m.create_database(explicit, s[1]).unwrap();
    assert_eq!(m.get_database(s[0]).unwrap().owner, s[2]);
}

#[test]
fn test_create_connection_limit_stored() {
    let (mut m, s) = setup(&["mydb", "postgres"]);
    let with_limit = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: false,
        owner: None,
        encoding: None,
        locale: None,
        tablespace: None,
        connection_limit: Some(50),
    };
    m.create_database(with_limit, s[1]).unwrap();
    assert_eq!(m.get_database(s[0]).unwrap().connection_limit, Some(50));
}

#[test]
fn test_create_multiple() {
    let (mut m, s) = setup(&["db1", "db2", "db3", "postgres"]);
    for &name in &s[..3] {
        m.create_database(stmt(name, false), s[3]).unwrap();
    }
    for &name in &s[..3] {
        assert!(m.database_exists(name));
    }
}

// ── database_exists ───────────────────────────────────────────────────────────

#[test]
fn test_exists_false_before_create() {
    let (m, s) = setup(&["ghost"]);
    assert!(!m.database_exists(s[0]));
}

// ── get_database ──────────────────────────────────────────────────────────────

#[test]
fn test_get_not_found() {
    let (m, s) = setup(&["ghost"]);
    assert_eq!(
        m.get_database(s[0]).unwrap_err(),
        CatalogError::DatabaseNotFound(s[0])
    );
}

#[test]
fn test_get_returns_correct_entry() {
    let (mut m, s) = setup(&["mydb", "postgres"]);
    m.create_database(stmt(s[0], false), s[1]).unwrap();
    let db = m.get_database(s[0]).unwrap();
    assert_eq!(db.name, s[0]);
    assert!(db.allow_connections);
    assert!(!db.is_template);
}

// ── drop_database ─────────────────────────────────────────────────────────────

#[test]
fn test_drop_success() {
    let (mut m, s) = setup(&["mydb", "postgres"]);
    m.create_database(stmt(s[0], false), s[1]).unwrap();
    assert!(m.drop_database(s[0], false).is_ok());
    assert!(!m.database_exists(s[0]));
}

#[test]
fn test_drop_not_found_error() {
    let (mut m, s) = setup(&["ghost"]);
    assert_eq!(
        m.drop_database(s[0], false).unwrap_err(),
        CatalogError::DatabaseNotFound(s[0])
    );
}

#[test]
fn test_drop_if_exists_silent() {
    let (mut m, s) = setup(&["ghost"]);
    assert!(m.drop_database(s[0], true).is_ok());
}

#[test]
fn test_drop_then_recreate() {
    let (mut m, s) = setup(&["mydb", "postgres"]);
    m.create_database(stmt(s[0], false), s[1]).unwrap();
    m.drop_database(s[0], false).unwrap();
    assert!(m.create_database(stmt(s[0], false), s[1]).is_ok());
}

// ── OID ───────────────────────────────────────────────────────────────────────

#[test]
fn test_oid_unique_per_database() {
    let (mut m, s) = setup(&["db1", "db2", "postgres"]);
    m.create_database(stmt(s[0], false), s[2]).unwrap();
    m.create_database(stmt(s[1], false), s[2]).unwrap();
    let oid1 = m.get_database(s[0]).unwrap().oid;
    let oid2 = m.get_database(s[1]).unwrap().oid;
    assert_ne!(oid1, oid2);
}

#[test]
fn test_oid_increments() {
    let (mut m, s) = setup(&["db1", "db2", "postgres"]);
    m.create_database(stmt(s[0], false), s[2]).unwrap();
    m.create_database(stmt(s[1], false), s[2]).unwrap();
    let oid1 = m.get_database(s[0]).unwrap().oid;
    let oid2 = m.get_database(s[1]).unwrap().oid;
    assert!(oid2 > oid1);
}
