use crate::{
    ast::{Statement, Value},
    binder::Binder,
    catalog::CatalogManager,
    common::Interner,
    executor::{ExecutionResult, Executor},
    parser::Parser,
    storage::Storage,
};
use std::time::Instant;

const ROW_COUNTS: &[usize] = &[100, 1_000, 10_000, 100_000, 10_00_000];

pub fn run_benchmarks() {
    for &n in ROW_COUNTS {
        println!("=== N = {} ===", n);
        bench_run(n);
        println!();
    }
}

fn bench_run(n: usize) {
    let _ = std::fs::remove_dir_all("./bench_data");

    let mut sql = String::from(
        "CREATE DATABASE benchdb; CREATE SCHEMA benchschema; \
         CREATE TABLE benchschema.users ( \
             id INT PRIMARY KEY, \
             name VARCHAR(255) NOT NULL, \
             email VARCHAR(255) UNIQUE \
         ); ",
    );
    sql.push_str(&build_inserts(0, n)); // unique
    sql.push_str(&build_inserts(0, n)); // same ids again -> PK conflicts
    sql.push_str("SELECT * FROM benchschema.users; ");
    sql.push_str(&format!(
        "SELECT * FROM benchschema.users WHERE id = {};",
        n / 2
    ));

    let mut parser = Parser::new(&sql);
    let stmts = match parser.parse() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            return;
        }
    };
    let interner = parser.interner;

    let catalog = CatalogManager::new(interner);
    let session_user = catalog.interner.intern("postgres");
    let current_db = catalog.interner.intern("benchdb");
    let public_schema = catalog.interner.intern("public");

    let storage = match Storage::new_or_create("./bench_data") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Storage error: {}", e);
            return;
        }
    };
    let mut executor = Executor::new(catalog, session_user, storage);

    let mut it = stmts.into_iter();

    // ── setup: CreateDatabase, CreateSchema, CreateTable ──
    for stmt in it.by_ref().take(3) {
        match stmt {
            Statement::CreateDatabase(s) => {
                let binder = Binder::new(&executor.catalog, executor.session_user);
                let bound = binder.bind_create_database(s).expect("bind create db");
                executor.execute_create_database(bound).expect("create db");
            }
            Statement::CreateSchema(s) => {
                let binder = Binder::new(&executor.catalog, executor.session_user);
                let bound = binder
                    .bind_create_schema(current_db, s)
                    .expect("bind create schema");
                executor
                    .execute_create_schema(current_db, bound)
                    .expect("create schema");
            }
            Statement::CreateTable(s) => {
                let binder = Binder::new(&executor.catalog, executor.session_user);
                let bound = binder
                    .bind_create_table(current_db, public_schema, s)
                    .expect("bind create table");
                executor.execute_create_table(bound).expect("create table");
            }
            _ => unreachable!("setup phase only has DDL"),
        }
    }

    // ── bulk unique insert ──
    let start = Instant::now();
    for stmt in it.by_ref().take(n) {
        if let Statement::Insert(s) = stmt {
            let binder = Binder::new(&executor.catalog, executor.session_user);
            let bound = binder
                .bind_insert_table(current_db, public_schema, s)
                .expect("bind insert");
            if let Err(e) = executor.execute_insert_table(bound) {
                eprintln!("insert error: {}", e.format(&executor.catalog.interner));
            }
        }
    }
    let elapsed = start.elapsed();
    println!(
        "Insert {} rows: {:?} ({:.2} rows/sec)",
        n,
        elapsed,
        n as f64 / elapsed.as_secs_f64()
    );

    // ── duplicate PK insert (same ids again) ──
    let start = Instant::now();
    let mut dup_errors = 0;
    for stmt in it.by_ref().take(n) {
        if let Statement::Insert(s) = stmt {
            let binder = Binder::new(&executor.catalog, executor.session_user);
            let bound = binder
                .bind_insert_table(current_db, public_schema, s)
                .expect("bind insert");
            if executor.execute_insert_table(bound).is_err() {
                dup_errors += 1;
            }
        }
    }
    let elapsed = start.elapsed();
    println!(
        "Duplicate-PK insert {} rows: {:?} ({} rejected)",
        n, elapsed, dup_errors
    );

    // ── select all ──
    if let Some(Statement::Select(s)) = it.next() {
        let binder = Binder::new(&executor.catalog, executor.session_user);
        let bound = binder
            .bind_select(current_db, public_schema, s)
            .expect("bind select");
        let start = Instant::now();
        let row_count = match executor.execute_select_table(bound) {
            Ok(ExecutionResult::Selected { rows }) => rows.len(),
            Ok(_) => unreachable!(),
            Err(e) => {
                eprintln!("select error: {}", e.format(&executor.catalog.interner));
                0
            }
        };
        println!("Select all ({} rows): {:?}", row_count, start.elapsed());
    }

    // ── select filtered ──
    if let Some(Statement::Select(s)) = it.next() {
        let binder = Binder::new(&executor.catalog, executor.session_user);
        let bound = binder
            .bind_select(current_db, public_schema, s)
            .expect("bind select");
        let start = Instant::now();
        let row_count = match executor.execute_select_table(bound) {
            Ok(ExecutionResult::Selected { rows }) => rows.len(),
            Ok(_) => unreachable!(),
            Err(e) => {
                eprintln!("select error: {}", e.format(&executor.catalog.interner));
                0
            }
        };
        println!(
            "Select filtered ({} rows): {:?}",
            row_count,
            start.elapsed()
        );
    }

    let _ = std::fs::remove_dir_all("./bench_data");
}

fn build_inserts(start_id: usize, n: usize) -> String {
    let mut sql = String::with_capacity(n * 90);
    for i in start_id..start_id + n {
        sql.push_str(&format!(
            "INSERT INTO benchschema.users (id, name, email) VALUES ({}, 'user{}', 'user{}@example.com'); ",
            i, i, i
        ));
    }
    sql
}

#[allow(dead_code)]
fn format_value(v: Value, interner: &Interner) -> String {
    match v {
        Value::Int(n) => n.to_string(),
        Value::Float(f) => f.to_string(),
        Value::String(sym) => interner.resolve(sym).to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Null => "NULL".to_string(),
        _ => "(unsupported)".to_string(),
    }
}
