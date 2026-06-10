use rust_sql::ast::CreateDatabaseStmt;
use rust_sql::binder::Binder;
use rust_sql::catalog::CatalogManager;
use rust_sql::common::interner::Interner;
use rust_sql::common::symbol::Symbol;
use rust_sql::executor::{ExecutionResult, Executor};

/// Builds an `Executor` and interns all provided names in one step.
/// Returns the executor and the interned symbols in the same order as `names`.
fn setup(names: &[&str]) -> (Executor, Vec<Symbol>) {
    let mut interner = Interner::new();
    let symbols: Vec<Symbol> = names.iter().map(|n| interner.intern(n)).collect();
    let catalog = CatalogManager::new(interner);
    let session = symbols[symbols.len() - 1]; // last name is always session user
    (Executor::new_in_memory(catalog, session), symbols)
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

/// Binds and executes a `CREATE DATABASE` statement end to end.
fn bind_and_execute(
    executor: &mut Executor,
    stmt: CreateDatabaseStmt,
) -> rust_sql::executor::error::ExecutionError {
    let binder = Binder::new(&executor.catalog, executor.session_user);
    let bound = binder.bind_create_database(stmt).expect("bind failed");
    executor.execute_create_database(bound).unwrap_err()
}

/// Binds and executes successfully — panics if either step fails.
fn bind_and_execute_ok(executor: &mut Executor, s: CreateDatabaseStmt) -> ExecutionResult {
    let binder = Binder::new(&executor.catalog, executor.session_user);
    let bound = binder.bind_create_database(s).expect("bind failed");
    executor
        .execute_create_database(bound)
        .expect("execute failed")
}

// ── Basic execution ───────────────────────────────────────────────────────────

#[test]
fn test_execute_minimal_success() {
    let (mut ex, s) = setup(&["mydb", "postgres"]);
    let result = bind_and_execute_ok(&mut ex, stmt(s[0], false));
    assert_eq!(result, ExecutionResult::DatabaseCreated { name: s[0] });
}

#[test]
fn test_execute_registers_in_catalog() {
    let (mut ex, s) = setup(&["mydb", "postgres"]);
    bind_and_execute_ok(&mut ex, stmt(s[0], false));
    // database must be visible in the catalog after execution
    assert!(ex.catalog.database_exists(s[0]));
}

#[test]
fn test_execute_command_tag() {
    let (mut ex, s) = setup(&["mydb", "postgres"]);
    let result = bind_and_execute_ok(&mut ex, stmt(s[0], false));
    assert_eq!(result.command_tag(), "CREATE DATABASE");
}

// ── IF NOT EXISTS ─────────────────────────────────────────────────────────────

#[test]
fn test_execute_if_not_exists_on_new_db() {
    let (mut ex, s) = setup(&["mydb", "postgres"]);
    // database does not exist yet — IF NOT EXISTS should still succeed
    let result = bind_and_execute_ok(&mut ex, stmt(s[0], true));
    assert_eq!(result, ExecutionResult::DatabaseCreated { name: s[0] });
}

#[test]
fn test_execute_if_not_exists_on_existing_db() {
    let (mut ex, s) = setup(&["mydb", "postgres"]);
    bind_and_execute_ok(&mut ex, stmt(s[0], false));
    // database now exists — IF NOT EXISTS should silently succeed
    let binder = Binder::new(&ex.catalog, ex.session_user);
    let bound = binder
        .bind_create_database(stmt(s[0], true))
        .expect("bind failed");
    let result = ex.execute_create_database(bound).expect("execute failed");
    assert_eq!(result, ExecutionResult::DatabaseCreated { name: s[0] });
}

// ── Owner ─────────────────────────────────────────────────────────────────────

#[test]
fn test_execute_owner_defaults_to_session_user() {
    let (mut ex, s) = setup(&["mydb", "postgres"]);
    bind_and_execute_ok(&mut ex, stmt(s[0], false));
    let db = ex.catalog.get_database(s[0]).unwrap();
    assert_eq!(db.owner, s[1]); // session user is postgres = s[1]
}

#[test]
fn test_execute_explicit_owner_stored() {
    let (mut ex, s) = setup(&["mydb", "postgres", "alice"]);
    let with_owner = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: false,
        owner: Some(s[2]),
        encoding: None,
        locale: None,
        tablespace: None,
        connection_limit: None,
    };
    bind_and_execute_ok(&mut ex, with_owner);
    let db = ex.catalog.get_database(s[0]).unwrap();
    assert_eq!(db.owner, s[2]);
}

// ── Catalog state after execution ─────────────────────────────────────────────

#[test]
fn test_execute_encoding_stored() {
    let (mut ex, s) = setup(&["mydb", "postgres", "UTF8"]);
    let with_encoding = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: false,
        owner: None,
        encoding: Some(s[2]),
        locale: None,
        tablespace: None,
        connection_limit: None,
    };
    bind_and_execute_ok(&mut ex, with_encoding);
    let db = ex.catalog.get_database(s[0]).unwrap();
    assert_eq!(db.encoding, Some(s[2]));
}

#[test]
fn test_execute_locale_stored() {
    let (mut ex, s) = setup(&["mydb", "postgres", "en_US"]);
    let with_locale = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: false,
        owner: None,
        encoding: None,
        locale: Some(s[2]),
        tablespace: None,
        connection_limit: None,
    };
    bind_and_execute_ok(&mut ex, with_locale);
    let db = ex.catalog.get_database(s[0]).unwrap();
    assert_eq!(db.locale, Some(s[2]));
}

#[test]
fn test_execute_connection_limit_stored() {
    let (mut ex, s) = setup(&["mydb", "postgres"]);
    let with_limit = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: false,
        owner: None,
        encoding: None,
        locale: None,
        tablespace: None,
        connection_limit: Some(50),
    };
    bind_and_execute_ok(&mut ex, with_limit);
    let db = ex.catalog.get_database(s[0]).unwrap();
    assert_eq!(db.connection_limit, Some(50));
}

#[test]
fn test_execute_full_statement_stored() {
    let (mut ex, s) = setup(&["mydb", "postgres", "alice", "UTF8", "en_US", "myspace"]);
    let full = CreateDatabaseStmt {
        name: s[0],
        if_not_exists: false,
        owner: Some(s[2]),
        encoding: Some(s[3]),
        locale: Some(s[4]),
        tablespace: Some(s[5]),
        connection_limit: Some(100),
    };
    bind_and_execute_ok(&mut ex, full);
    let db = ex.catalog.get_database(s[0]).unwrap();
    assert_eq!(db.name, s[0]);
    assert_eq!(db.owner, s[2]);
    assert_eq!(db.encoding, Some(s[3]));
    assert_eq!(db.locale, Some(s[4]));
    assert_eq!(db.tablespace, Some(s[5]));
    assert_eq!(db.connection_limit, Some(100));
}

// ── Multiple databases ────────────────────────────────────────────────────────

#[test]
fn test_execute_multiple_databases() {
    let (mut ex, s) = setup(&["db1", "db2", "db3", "postgres"]);
    for &name in &s[..3] {
        bind_and_execute_ok(&mut ex, stmt(name, false));
    }
    for &name in &s[..3] {
        assert!(ex.catalog.database_exists(name));
    }
}

#[test]
fn test_execute_oid_increments() {
    let (mut ex, s) = setup(&["db1", "db2", "postgres"]);
    bind_and_execute_ok(&mut ex, stmt(s[0], false));
    bind_and_execute_ok(&mut ex, stmt(s[1], false));
    let oid1 = ex.catalog.get_database(s[0]).unwrap().oid;
    let oid2 = ex.catalog.get_database(s[1]).unwrap().oid;
    assert!(oid2 > oid1);
}

// ── Catalog defaults ──────────────────────────────────────────────────────────

#[test]
fn test_execute_allow_connections_defaults_true() {
    let (mut ex, s) = setup(&["mydb", "postgres"]);
    bind_and_execute_ok(&mut ex, stmt(s[0], false));
    let db = ex.catalog.get_database(s[0]).unwrap();
    assert!(db.allow_connections);
}

#[test]
fn test_execute_is_template_defaults_false() {
    let (mut ex, s) = setup(&["mydb", "postgres"]);
    bind_and_execute_ok(&mut ex, stmt(s[0], false));
    let db = ex.catalog.get_database(s[0]).unwrap();
    assert!(!db.is_template);
}

// ── Storage integration ───────────────────────────────────────────────────────

#[test]
fn test_execute_creates_database_directory() {
    use rust_sql::storage::Storage;
    use std::path::PathBuf;

    let tmp = std::env::temp_dir().join("rust_sql_test_create_db");
    let _ = std::fs::remove_dir_all(&tmp); // clean up from previous runs
    std::fs::create_dir_all(&tmp).unwrap();

    let mut interner = Interner::new();
    let name = interner.intern("mydb");
    let session = interner.intern("postgres");
    let catalog = CatalogManager::new(interner);
    let storage = Storage::new(&tmp).unwrap();
    let mut executor = Executor::new(catalog, session, storage);

    let binder = Binder::new(&executor.catalog, session);
    let bound = binder.bind_create_database(stmt(name, false)).unwrap();
    executor.execute_create_database(bound).unwrap();

    // database directory must exist
    assert!(tmp.join("mydb").exists());
    // default public schema directory must exist
    assert!(tmp.join("mydb").join("public").exists());

    std::fs::remove_dir_all(&tmp).unwrap(); // cleanup
}

#[test]
fn test_execute_in_memory_skips_storage() {
    // new_in_memory should succeed without touching disk
    let (mut ex, s) = setup(&["mydb", "postgres"]);
    let result = bind_and_execute_ok(&mut ex, stmt(s[0], false));
    assert_eq!(result.command_tag(), "CREATE DATABASE");
    // no directory created — nothing to assert on disk
}
