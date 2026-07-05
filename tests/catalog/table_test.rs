use osirisdb::ast::DataType;
use osirisdb::ast::{CreateDatabaseStmt, CreateSchemaStmt};
use osirisdb::catalog::objects::ColumnEntry;
use osirisdb::catalog::{CatalogError, CatalogManager};
use osirisdb::common::interner::Interner;
use osirisdb::common::symbol::Symbol;
use osirisdb::storage::Storage;
use std::env;
use std::path::PathBuf;

struct TestStorage {
    path: PathBuf,
    storage: Storage,
}

impl TestStorage {
    fn new(name: &str) -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let count = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = env::temp_dir().join(format!("osirisdb_catalog_table_test_{}_{}", name, count));
        let _ = std::fs::remove_dir_all(&path);
        let storage = Storage::new_or_create(&path).unwrap();
        Self { path, storage }
    }
}

impl Drop for TestStorage {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

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

#[test]
fn test_create_table_success() {
    let (mut m, s) = setup(&["mydb", "myschema", "mytable", "col1", "postgres"]);
    let ts = TestStorage::new("create_table_success");
    ts.storage.create_database_dir("mydb").unwrap();
    ts.storage.create_schema_dir("mydb", "myschema").unwrap();

    m.create_database(create_db_stmt(s[0]), s[4]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1])), s[4])
        .unwrap();

    let columns = vec![ColumnEntry {
        name: s[3],
        data_type: DataType::Int,
        nullable: true,
        default: None,
        is_unique: false,
        is_primary_key: false,
    }];
    assert!(
        m.create_table(
            &ts.storage,
            s[0],
            s[1],
            s[2],
            columns.clone(),
            vec![],
            false
        )
        .is_ok()
    );
    assert!(m.table_exists(s[0], s[1], s[2]));

    let table = m.get_table(s[0], s[1], s[2]).unwrap();
    assert_eq!(table.name, s[2]);
    assert_eq!(table.columns, columns);
}

#[test]
fn test_create_table_schema_not_found() {
    let (mut m, s) = setup(&["mydb", "myschema", "mytable", "postgres"]);
    let ts = TestStorage::new("create_table_schema_not_found");
    ts.storage.create_database_dir("mydb").unwrap();
    m.create_database(create_db_stmt(s[0]), s[3]).unwrap();

    let err = m
        .create_table(&ts.storage, s[0], s[1], s[2], vec![], vec![], false)
        .unwrap_err();
    assert_eq!(err, CatalogError::SchemaNotFound(s[1]));
}

#[test]
fn test_create_table_already_exists_error() {
    let (mut m, s) = setup(&["mydb", "myschema", "mytable", "postgres"]);
    let ts = TestStorage::new("create_table_already_exists_error");
    ts.storage.create_database_dir("mydb").unwrap();
    ts.storage.create_schema_dir("mydb", "myschema").unwrap();

    m.create_database(create_db_stmt(s[0]), s[3]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1])), s[3])
        .unwrap();

    m.create_table(&ts.storage, s[0], s[1], s[2], vec![], vec![], false)
        .unwrap();
    let err = m
        .create_table(&ts.storage, s[0], s[1], s[2], vec![], vec![], false)
        .unwrap_err();
    assert_eq!(err, CatalogError::TableAlreadyExists(s[2]));
}

#[test]
fn test_create_table_if_not_exists_silent() {
    let (mut m, s) = setup(&["mydb", "myschema", "mytable", "postgres"]);
    let ts = TestStorage::new("create_table_if_not_exists_silent");
    ts.storage.create_database_dir("mydb").unwrap();
    ts.storage.create_schema_dir("mydb", "myschema").unwrap();

    m.create_database(create_db_stmt(s[0]), s[3]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1])), s[3])
        .unwrap();

    m.create_table(&ts.storage, s[0], s[1], s[2], vec![], vec![], false)
        .unwrap();
    assert!(
        m.create_table(&ts.storage, s[0], s[1], s[2], vec![], vec![], true)
            .is_ok()
    );
}

#[test]
fn test_get_table_not_found() {
    let (mut m, s) = setup(&["mydb", "myschema", "mytable", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[3]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1])), s[3])
        .unwrap();

    let err = m.get_table(s[0], s[1], s[2]).unwrap_err();
    assert_eq!(err, CatalogError::TableNotFound(s[2]));
}

#[test]
fn test_table_oid_increments() {
    let (mut m, s) = setup(&["mydb", "myschema", "t1", "t2", "postgres"]);
    let ts = TestStorage::new("table_oid_increments");
    ts.storage.create_database_dir("mydb").unwrap();
    ts.storage.create_schema_dir("mydb", "myschema").unwrap();

    m.create_database(create_db_stmt(s[0]), s[4]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1])), s[4])
        .unwrap();

    m.create_table(&ts.storage, s[0], s[1], s[2], vec![], vec![], false)
        .unwrap();
    m.create_table(&ts.storage, s[0], s[1], s[3], vec![], vec![], false)
        .unwrap();

    let oid1 = m.get_table(s[0], s[1], s[2]).unwrap().oid;
    let oid2 = m.get_table(s[0], s[1], s[3]).unwrap().oid;
    assert!(oid2 > oid1);
}
