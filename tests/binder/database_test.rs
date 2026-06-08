use rust_sql::ast::CreateDatabaseStmt;
use rust_sql::binder::{BindError, Binder};
use rust_sql::catalog::CatalogManager;
use rust_sql::common::interner::Interner;
use rust_sql::common::symbol::Symbol;

/// Builds a `CatalogManager` and interns all provided names in one step.
fn setup(names: &[&str]) -> (CatalogManager, Vec<Symbol>) {
    let mut interner = Interner::new();
    let symbols = names.iter().map(|n| interner.intern(n)).collect();
    (CatalogManager::new(interner), symbols)
}

/// Minimal `CreateDatabaseStmt` with only name and if_not_exists set.
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

// ── Basic binding ─────────────────────────────────────────────────────────────

#[test]
fn test_bind_minimal_success() {
    let (m, s) = setup(&["mydb", "postgres"]);
    let binder = Binder::new(&m, s[1]);
    assert!(binder.bind_create_database(stmt(s[0], false)).is_ok());
}

#[test]
fn test_bind_if_not_exists_success() {
    let (m, s) = setup(&["mydb", "postgres"]);
    let binder = Binder::new(&m, s[1]);
    assert!(binder.bind_create_database(stmt(s[0], true)).is_ok());
}

// ── Owner resolution ──────────────────────────────────────────────────────────

#[test]
fn test_owner_defaults_to_session_user() {
    let (m, s) = setup(&["mydb", "postgres"]);
    let binder = Binder::new(&m, s[1]);
    let bound = binder.bind_create_database(stmt(s[0], false)).unwrap();
    // owner not specified — should resolve to session user
    assert_eq!(bound.owner, s[1]);
}

#[test]
fn test_explicit_owner_preserved() {
    let (m, s) = setup(&["mydb", "postgres", "alice"]);
    let binder = Binder::new(&m, s[1]);
    let with_owner = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: false,
        owner: Some(s[2]),
        encoding: None,
        locale: None,
        tablespace: None,
        connection_limit: None,
    };
    let bound = binder.bind_create_database(with_owner).unwrap();
    // explicit owner — should not be overridden by session user
    assert_eq!(bound.owner, s[2]);
}

// ── Existence validation ──────────────────────────────────────────────────────

#[test]
fn test_fails_if_database_already_exists() {
    let (mut m, s) = setup(&["mydb", "postgres"]);
    // create the database first so it exists in the catalog
    m.create_database(stmt(s[0], false), s[1]).unwrap();
    let binder = Binder::new(&m, s[1]);
    let err = binder.bind_create_database(stmt(s[0], false)).unwrap_err();
    assert_eq!(err, BindError::DatabaseAlreadyExists(s[0]));
}

#[test]
fn test_if_not_exists_skips_existence_check() {
    let (mut m, s) = setup(&["mydb", "postgres"]);
    m.create_database(stmt(s[0], false), s[1]).unwrap();
    let binder = Binder::new(&m, s[1]);
    // IF NOT EXISTS — binder should pass even though database exists
    assert!(binder.bind_create_database(stmt(s[0], true)).is_ok());
}

// ── Connection limit validation ───────────────────────────────────────────────

#[test]
fn test_connection_limit_zero_valid() {
    let (m, s) = setup(&["mydb", "postgres"]);
    let binder = Binder::new(&m, s[1]);
    let with_limit = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: false,
        owner: None,
        encoding: None,
        locale: None,
        tablespace: None,
        connection_limit: Some(0),
    };
    assert!(binder.bind_create_database(with_limit).is_ok());
}

#[test]
fn test_connection_limit_positive_valid() {
    let (m, s) = setup(&["mydb", "postgres"]);
    let binder = Binder::new(&m, s[1]);
    let with_limit = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: false,
        owner: None,
        encoding: None,
        locale: None,
        tablespace: None,
        connection_limit: Some(100),
    };
    assert!(binder.bind_create_database(with_limit).is_ok());
}

#[test]
fn test_connection_limit_negative_one_valid() {
    let (m, s) = setup(&["mydb", "postgres"]);
    let binder = Binder::new(&m, s[1]);
    let with_limit = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: false,
        owner: None,
        encoding: None,
        locale: None,
        tablespace: None,
        connection_limit: Some(-1), // -1 = unlimited, always valid
    };
    assert!(binder.bind_create_database(with_limit).is_ok());
}

#[test]
fn test_connection_limit_below_negative_one_invalid() {
    let (m, s) = setup(&["mydb", "postgres"]);
    let binder = Binder::new(&m, s[1]);
    let with_limit = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: false,
        owner: None,
        encoding: None,
        locale: None,
        tablespace: None,
        connection_limit: Some(-2),
    };
    let err = binder.bind_create_database(with_limit).unwrap_err();
    assert_eq!(err, BindError::InvalidConnectionLimit(-2));
}

#[test]
fn test_connection_limit_large_negative_invalid() {
    let (m, s) = setup(&["mydb", "postgres"]);
    let binder = Binder::new(&m, s[1]);
    let with_limit = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: false,
        owner: None,
        encoding: None,
        locale: None,
        tablespace: None,
        connection_limit: Some(-100),
    };
    let err = binder.bind_create_database(with_limit).unwrap_err();
    assert_eq!(err, BindError::InvalidConnectionLimit(-100));
}

// ── Bound output correctness ──────────────────────────────────────────────────

#[test]
fn test_bound_preserves_name() {
    let (m, s) = setup(&["mydb", "postgres"]);
    let binder = Binder::new(&m, s[1]);
    let bound = binder.bind_create_database(stmt(s[0], false)).unwrap();
    assert_eq!(bound.name, s[0]);
}

#[test]
fn test_bound_preserves_if_not_exists() {
    let (m, s) = setup(&["mydb", "postgres"]);
    let binder = Binder::new(&m, s[1]);
    let bound = binder.bind_create_database(stmt(s[0], true)).unwrap();
    assert!(bound.if_not_exists);
}

#[test]
fn test_bound_preserves_encoding() {
    let (m, s) = setup(&["mydb", "postgres", "UTF8"]);
    let binder = Binder::new(&m, s[1]);
    let with_encoding = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: false,
        owner: None,
        encoding: Some(s[2]),
        locale: None,
        tablespace: None,
        connection_limit: None,
    };
    let bound = binder.bind_create_database(with_encoding).unwrap();
    assert_eq!(bound.encoding, Some(s[2]));
}

#[test]
fn test_bound_preserves_locale() {
    let (m, s) = setup(&["mydb", "postgres", "en_US"]);
    let binder = Binder::new(&m, s[1]);
    let with_locale = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: false,
        owner: None,
        encoding: None,
        locale: Some(s[2]),
        tablespace: None,
        connection_limit: None,
    };
    let bound = binder.bind_create_database(with_locale).unwrap();
    assert_eq!(bound.locale, Some(s[2]));
}

#[test]
fn test_bound_preserves_connection_limit() {
    let (m, s) = setup(&["mydb", "postgres"]);
    let binder = Binder::new(&m, s[1]);
    let with_limit = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: false,
        owner: None,
        encoding: None,
        locale: None,
        tablespace: None,
        connection_limit: Some(50),
    };
    let bound = binder.bind_create_database(with_limit).unwrap();
    assert_eq!(bound.connection_limit, Some(50));
}

#[test]
fn test_bound_full_statement() {
    let (m, s) = setup(&["mydb", "postgres", "alice", "UTF8", "en_US", "myspace"]);
    let binder = Binder::new(&m, s[1]);
    let full = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: true,
        owner: Some(s[2]),
        encoding: Some(s[3]),
        locale: Some(s[4]),
        tablespace: Some(s[5]),
        connection_limit: Some(50),
    };
    let bound = binder.bind_create_database(full).unwrap();
    assert_eq!(bound.name, s[0]);
    assert!(bound.if_not_exists);
    assert_eq!(bound.owner, s[2]);
    assert_eq!(bound.encoding, Some(s[3]));
    assert_eq!(bound.locale, Some(s[4]));
    assert_eq!(bound.tablespace, Some(s[5]));
    assert_eq!(bound.connection_limit, Some(50));
}
