use osirisdb::ast::{
    ColumnDef, CreateDatabaseStmt, CreateSchemaStmt, CreateTableStmt, DataType, ObjectName,
};
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

fn bind_and_execute_table_ok(
    executor: &mut Executor,
    db: Symbol,
    schema: Symbol,
    s: CreateTableStmt,
) -> ExecutionResult {
    let binder = Binder::new(&executor.catalog, executor.session_user);
    let bound = binder
        .bind_create_table(db, schema, s)
        .expect("bind failed");
    executor
        .execute_create_table(bound)
        .expect("execute failed")
}

#[test]
fn test_execute_table_minimal_success() {
    let (mut ex, s) = setup(&["mydb", "myschema", "mytable", "col1", "postgres"]);
    // Create database and schema first
    let db_bound = Binder::new(&ex.catalog, ex.session_user)
        .bind_create_database(create_db_stmt(s[0]))
        .unwrap();
    ex.execute_create_database(db_bound).unwrap();

    let schema_bound = Binder::new(&ex.catalog, ex.session_user)
        .bind_create_schema(s[0], schema_stmt(Some(s[1])))
        .unwrap();
    ex.execute_create_schema(s[0], schema_bound).unwrap();

    let columns = vec![column_def(s[3], DataType::Int)];
    let obj_name = ObjectName(vec![s[1], s[2]]);
    let result =
        bind_and_execute_table_ok(&mut ex, s[0], s[1], table_stmt(obj_name, columns, false));

    assert_eq!(result, ExecutionResult::TableCreated { name: s[2] });
    assert!(ex.catalog.table_exists(s[0], s[1], s[2]));
}

#[test]
fn test_execute_table_command_tag() {
    let (mut ex, s) = setup(&["mydb", "myschema", "mytable", "col1", "postgres"]);
    let db_bound = Binder::new(&ex.catalog, ex.session_user)
        .bind_create_database(create_db_stmt(s[0]))
        .unwrap();
    ex.execute_create_database(db_bound).unwrap();
    let schema_bound = Binder::new(&ex.catalog, ex.session_user)
        .bind_create_schema(s[0], schema_stmt(Some(s[1])))
        .unwrap();
    ex.execute_create_schema(s[0], schema_bound).unwrap();

    let columns = vec![column_def(s[3], DataType::Int)];
    let obj_name = ObjectName(vec![s[1], s[2]]);
    let result =
        bind_and_execute_table_ok(&mut ex, s[0], s[1], table_stmt(obj_name, columns, false));
    assert_eq!(result.command_tag(), "CREATE TABLE");
}

#[test]
fn test_execute_table_if_not_exists() {
    let (mut ex, s) = setup(&["mydb", "myschema", "mytable", "col1", "postgres"]);
    let db_bound = Binder::new(&ex.catalog, ex.session_user)
        .bind_create_database(create_db_stmt(s[0]))
        .unwrap();
    ex.execute_create_database(db_bound).unwrap();
    let schema_bound = Binder::new(&ex.catalog, ex.session_user)
        .bind_create_schema(s[0], schema_stmt(Some(s[1])))
        .unwrap();
    ex.execute_create_schema(s[0], schema_bound).unwrap();

    let columns = vec![column_def(s[3], DataType::Int)];
    let obj_name = ObjectName(vec![s[1], s[2]]);
    bind_and_execute_table_ok(
        &mut ex,
        s[0],
        s[1],
        table_stmt(obj_name.clone(), columns.clone(), false),
    );

    // Execute again with IF NOT EXISTS
    let binder = Binder::new(&ex.catalog, ex.session_user);
    let bound = binder
        .bind_create_table(s[0], s[1], table_stmt(obj_name, columns, true))
        .unwrap();
    let result = ex.execute_create_table(bound).unwrap();
    assert_eq!(result, ExecutionResult::TableCreated { name: s[2] });
}

#[test]
fn test_execute_table_creates_file() {
    use osirisdb::storage::Storage;

    let tmp = std::env::temp_dir().join("osirisdb_test_create_table");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let mut interner = Interner::new();
    let db = interner.intern("mydb");
    let schema = interner.intern("myschema");
    let table = interner.intern("mytable");
    let col = interner.intern("col1");
    let session = interner.intern("postgres");
    let catalog = CatalogManager::new(interner);
    let storage = Storage::new(&tmp).unwrap();
    let mut executor = Executor::new(catalog, session, storage);

    // Create database and schema
    let db_bound = Binder::new(&executor.catalog, session)
        .bind_create_database(create_db_stmt(db))
        .unwrap();
    executor.execute_create_database(db_bound).unwrap();
    let schema_bound = Binder::new(&executor.catalog, session)
        .bind_create_schema(db, schema_stmt(Some(schema)))
        .unwrap();
    executor.execute_create_schema(db, schema_bound).unwrap();

    // Create table
    let columns = vec![column_def(col, DataType::Int)];
    let obj_name = ObjectName(vec![schema, table]);
    let binder = Binder::new(&executor.catalog, session);
    let bound = binder
        .bind_create_table(db, schema, table_stmt(obj_name, columns, false))
        .unwrap();
    executor.execute_create_table(bound).unwrap();

    // Verify file exists
    assert!(
        tmp.join("mydb")
            .join("myschema")
            .join("mytable.dat")
            .exists()
    );

    std::fs::remove_dir_all(&tmp).unwrap();
}
