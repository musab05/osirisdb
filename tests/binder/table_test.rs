use osirisdb::ast::{
    ColumnConstraint, ColumnDef, CreateDatabaseStmt, CreateSchemaStmt, CreateTableStmt, DataType,
    Expr, ObjectName, TableConstraint, Value,
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
    assert!(bound.columns[0].nullable); // SQL default is nullable
    assert_eq!(bound.columns[0].default, None);
    assert!(!bound.columns[0].is_unique);
    assert!(!bound.columns[0].is_primary_key);
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
    m.create_table(s[0], s[1], s[2], vec![], vec![], false)
        .unwrap();

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

#[test]
fn test_bind_table_duplicate_column() {
    let (mut m, s) = setup(&["mydb", "myschema", "mytable", "col1", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[4]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1])), s[4])
        .unwrap();

    let binder = Binder::new(&m, s[4]);
    let columns = vec![
        column_def(s[3], DataType::Int),
        column_def(s[3], DataType::Text), // duplicate name
    ];
    let obj_name = ObjectName(vec![s[1], s[2]]);
    let err = binder
        .bind_create_table(s[0], s[1], table_stmt(obj_name, columns, false))
        .unwrap_err();
    assert_eq!(err, BindError::DuplicateColumnName(s[3]));
}

#[test]
fn test_bind_table_multiple_primary_keys() {
    let (mut m, s) = setup(&["mydb", "myschema", "mytable", "col1", "col2", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[5]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1])), s[5])
        .unwrap();

    let binder = Binder::new(&m, s[5]);
    let mut c1 = column_def(s[3], DataType::Int);
    c1.constraints.push(ColumnConstraint::PrimaryKey);
    let mut c2 = column_def(s[4], DataType::Int);
    c2.constraints.push(ColumnConstraint::PrimaryKey);

    let columns = vec![c1, c2];
    let obj_name = ObjectName(vec![s[1], s[2]]);
    let err = binder
        .bind_create_table(s[0], s[1], table_stmt(obj_name, columns, false))
        .unwrap_err();
    assert_eq!(err, BindError::MultiplePrimaryKeys);
}

#[test]
fn test_bind_table_column_not_found_in_constraint() {
    let (mut m, s) = setup(&["mydb", "myschema", "mytable", "col1", "col2", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[5]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1])), s[5])
        .unwrap();

    let binder = Binder::new(&m, s[5]);
    let columns = vec![column_def(s[3], DataType::Int)];
    let obj_name = ObjectName(vec![s[1], s[2]]);

    // table constraint referencing col2 which is not in columns
    let mut stmt = table_stmt(obj_name, columns, false);
    stmt.constraints.push(TableConstraint::PrimaryKey {
        name: None,
        columns: vec![s[4]], // col2
    });

    let err = binder.bind_create_table(s[0], s[1], stmt).unwrap_err();
    assert_eq!(err, BindError::ColumnNotFound(s[4]));
}

#[test]
fn test_bind_table_column_level_constraints() {
    let (mut m, s) = setup(&["mydb", "myschema", "mytable", "col1", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[4]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1])), s[4])
        .unwrap();

    let binder = Binder::new(&m, s[4]);
    let mut c1 = column_def(s[3], DataType::Int);
    c1.constraints.push(ColumnConstraint::NotNull);
    c1.constraints.push(ColumnConstraint::Unique);
    c1.constraints
        .push(ColumnConstraint::Default(Expr::Literal(Value::Int(42))));

    let columns = vec![c1];
    let obj_name = ObjectName(vec![s[1], s[2]]);
    let bound = binder
        .bind_create_table(s[0], s[1], table_stmt(obj_name, columns, false))
        .unwrap();

    assert!(!bound.columns[0].nullable);
    assert!(bound.columns[0].is_unique);
    assert_eq!(
        bound.columns[0].default,
        Some(Expr::Literal(Value::Int(42)))
    );
}

#[test]
fn test_bind_table_constraint_folding_single_pk() {
    let (mut m, s) = setup(&["mydb", "myschema", "mytable", "col1", "col2", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[5]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1])), s[5])
        .unwrap();

    let binder = Binder::new(&m, s[5]);
    let columns = vec![
        column_def(s[3], DataType::Int),
        column_def(s[4], DataType::Int),
    ];
    let obj_name = ObjectName(vec![s[1], s[2]]);

    let mut stmt = table_stmt(obj_name, columns, false);
    stmt.constraints.push(TableConstraint::PrimaryKey {
        name: None,
        columns: vec![s[3]], // col1
    });
    stmt.constraints.push(TableConstraint::Unique {
        name: None,
        columns: vec![s[4]], // col2
    });

    let bound = binder.bind_create_table(s[0], s[1], stmt).unwrap();

    // Verify single-column constraints were folded into the ColumnEntry flags
    assert!(bound.columns[0].is_primary_key);
    assert!(!bound.columns[0].nullable);
    assert!(bound.columns[1].is_unique);
    // Table constraints should be empty since they were folded
    assert!(bound.constraints.is_empty());
}

#[test]
fn test_bind_table_constraint_composite_pk() {
    let (mut m, s) = setup(&["mydb", "myschema", "mytable", "col1", "col2", "postgres"]);
    m.create_database(create_db_stmt(s[0]), s[5]).unwrap();
    m.create_schema(s[0], schema_stmt(Some(s[1])), s[5])
        .unwrap();

    let binder = Binder::new(&m, s[5]);
    let columns = vec![
        column_def(s[3], DataType::Int),
        column_def(s[4], DataType::Int),
    ];
    let obj_name = ObjectName(vec![s[1], s[2]]);

    let mut stmt = table_stmt(obj_name, columns, false);
    stmt.constraints.push(TableConstraint::PrimaryKey {
        name: None,
        columns: vec![s[3], s[4]], // composite PK on col1 and col2
    });

    let bound = binder.bind_create_table(s[0], s[1], stmt).unwrap();

    // Composite primary key should set flags on all participating columns
    assert!(bound.columns[0].is_primary_key);
    assert!(!bound.columns[0].nullable);
    assert!(bound.columns[1].is_primary_key);
    assert!(!bound.columns[1].nullable);

    // The table constraint should NOT be folded since it is composite
    assert_eq!(bound.constraints.len(), 1);
    assert_eq!(
        bound.constraints[0],
        TableConstraint::PrimaryKey {
            name: None,
            columns: vec![s[3], s[4]],
        }
    );
}
