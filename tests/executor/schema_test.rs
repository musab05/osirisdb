use osirisdb::ast::{CreateDatabaseStmt, CreateSchemaStmt};
use osirisdb::binder::Binder;
use osirisdb::catalog::CatalogManager;
use osirisdb::common::interner::Interner;
use osirisdb::common::symbol::Symbol;
use osirisdb::executor::{ExecutionResult, Executor};

fn setup(names: &[&str]) -> (Executor, Vec<Symbol>) {
    let mut interner = Interner::new();
    let symbols: Vec<Symbol> = names.iter().map(|n| interner.intern(n)).collect();
    let catalog = CatalogManager::new(interner);
    let session = symbols[symbols.len() - 1];
    (Executor::new_in_memory(catalog, session), symbols)
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

fn bind_and_execute_schema_ok(
    executor: &mut Executor,
    db: Symbol,
    s: CreateSchemaStmt,
) -> ExecutionResult {
    let binder = Binder::new(&executor.catalog, executor.session_user);
    let bound = binder.bind_create_schema(db, s).expect("bind failed");
    executor
        .execute_create_schema(db, bound)
        .expect("execute failed")
}

#[test]
fn test_execute_schema_minimal_success() {
    let (mut ex, s) = setup(&["mydb", "myschema", "postgres"]);
    // Create database first
    let db_binder = Binder::new(&ex.catalog, ex.session_user);
    let db_bound = db_binder
        .bind_create_database(create_db_stmt(s[0]))
        .unwrap();
    ex.execute_create_database(db_bound).unwrap();

    let result = bind_and_execute_schema_ok(&mut ex, s[0], schema_stmt(Some(s[1]), false));
    assert_eq!(result, ExecutionResult::SchemaCreated { name: s[1] });
    assert!(ex.catalog.schema_exists(s[0], s[1]));
}

#[test]
fn test_execute_schema_command_tag() {
    let (mut ex, s) = setup(&["mydb", "myschema", "postgres"]);
    let db_binder = Binder::new(&ex.catalog, ex.session_user);
    let db_bound = db_binder
        .bind_create_database(create_db_stmt(s[0]))
        .unwrap();
    ex.execute_create_database(db_bound).unwrap();

    let result = bind_and_execute_schema_ok(&mut ex, s[0], schema_stmt(Some(s[1]), false));
    assert_eq!(result.command_tag(), "CREATE SCHEMA");
}

#[test]
fn test_execute_schema_if_not_exists() {
    let (mut ex, s) = setup(&["mydb", "myschema", "postgres"]);
    let db_binder = Binder::new(&ex.catalog, ex.session_user);
    let db_bound = db_binder
        .bind_create_database(create_db_stmt(s[0]))
        .unwrap();
    ex.execute_create_database(db_bound).unwrap();

    bind_and_execute_schema_ok(&mut ex, s[0], schema_stmt(Some(s[1]), false));

    // Execute again with IF NOT EXISTS
    let binder = Binder::new(&ex.catalog, ex.session_user);
    let bound = binder
        .bind_create_schema(s[0], schema_stmt(Some(s[1]), true))
        .unwrap();
    let result = ex.execute_create_schema(s[0], bound).unwrap();
    assert_eq!(result, ExecutionResult::SchemaCreated { name: s[1] });
}

#[test]
fn test_execute_schema_creates_directory() {
    use osirisdb::storage::Storage;

    let tmp = std::env::temp_dir().join("osirisdb_test_create_schema");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let mut interner = Interner::new();
    let db = interner.intern("mydb");
    let schema = interner.intern("myschema");
    let session = interner.intern("postgres");
    let catalog = CatalogManager::new(interner);
    let storage = Storage::new(&tmp).unwrap();
    let mut executor = Executor::new(catalog, session, storage);

    // Create database first
    let db_binder = Binder::new(&executor.catalog, session);
    let db_bound = db_binder.bind_create_database(create_db_stmt(db)).unwrap();
    executor.execute_create_database(db_bound).unwrap();

    // Create schema
    let binder = Binder::new(&executor.catalog, session);
    let bound = binder
        .bind_create_schema(db, schema_stmt(Some(schema), false))
        .unwrap();
    executor.execute_create_schema(db, bound).unwrap();

    // Verify directory exists
    assert!(tmp.join("mydb").join("myschema").exists());

    std::fs::remove_dir_all(&tmp).unwrap();
}
