use crate::{
    ast::{Statement, Value},
    binder::Binder,
    catalog::CatalogManager,
    common::Interner,
    executor::{ExecutionResult, Executor},
    parser::Parser,
    storage::Storage,
};
use std::io::{self, Write};

pub fn sql() {
    let storage = match Storage::new_or_create("./data") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Storage error: {}", e);
            return;
        }
    };

    let interner = Interner::new();
    let session_user = interner.intern("postgres");
    let mut current_db = interner.intern("mydb");
    let public_schema = interner.intern("public");

    // Initialize CatalogManager
    let mut catalog = CatalogManager::new(interner);

    // Ensure default database 'mydb' exists in catalog and disk
    if catalog.get_database(current_db).is_err() {
        let create_stmt = crate::ast::CreateDatabaseStmt {
            name: current_db,
            if_not_exists: true,
            owner: Some(session_user),
            encoding: None,
            locale: None,
            tablespace: None,
            connection_limit: None,
        };
        if let Err(e) = catalog.create_database(create_stmt, session_user) {
            eprintln!("Failed to create default database catalog: {:?}", e);
        }
        if let Err(e) = storage.create_database_dir("mydb") {
            eprintln!("Failed to create default database directory: {}", e);
        }
    }

    let mut executor = Executor::new(catalog, session_user, storage);

    println!("Welcome to OsirisDB CLI!");
    println!("Type your SQL statements. End statements with a semicolon ';'.");
    println!("Type '\\q', 'exit', or 'quit' to quit.");
    println!();

    let mut input_buffer = String::new();

    loop {
        // Show prompt based on input buffer state
        if input_buffer.trim().is_empty() {
            let db_name = executor.catalog.interner.resolve(current_db);
            print!("{}# ", db_name);
        } else {
            print!("{}~# ", executor.catalog.interner.resolve(current_db));
        }
        let _ = io::stdout().flush();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            eprintln!("Error reading input");
            break;
        }

        let trimmed = line.trim();
        if trimmed == "\\q" || trimmed == "exit" || trimmed == "quit" {
            println!("Goodbye!");
            break;
        }

        input_buffer.push_str(&line);

        // Keep reading if the statement is not finished with a semicolon
        if !input_buffer.trim().ends_with(';') {
            continue;
        }

        let sql_to_parse = input_buffer.clone();
        input_buffer.clear(); // Reset the buffer for the next statement

        let mut parser = Parser::new(&sql_to_parse);
        // Sync catalog interner into parser so existing symbols resolve correctly
        parser.interner = executor.catalog.interner.clone();

        let stmts = match parser.parse() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Parse error: {}", e);
                continue;
            }
        };

        // Sync parser's interner back into executor's catalog
        executor.catalog.interner = parser.interner;

        for stmt in stmts {
            match stmt {
                Statement::UseDatabase(s) => {
                    let start = std::time::Instant::now();
                    // Check if database exists in catalog
                    if executor.catalog.get_database(s.database_name).is_ok() {
                        current_db = s.database_name;
                        let db_name = executor.catalog.interner.resolve(current_db);
                        println!("Switched to database '{}'", db_name);
                    } else {
                        let db_name_str = executor.catalog.interner.resolve(s.database_name);
                        eprintln!(
                            "Execution error: database \"{}\" does not exist",
                            db_name_str
                        );
                    }
                    println!("Time: {:?}", start.elapsed());
                }
                Statement::CreateDatabase(s) => {
                    let binder = Binder::new(&executor.catalog, executor.session_user);
                    let bound = match binder.bind_create_database(s) {
                        Ok(b) => b,
                        Err(e) => {
                            eprintln!("Bind error: {}", e.format(&executor.catalog.interner));
                            continue;
                        }
                    };

                    let start = std::time::Instant::now();
                    match executor.execute_create_database(bound) {
                        Ok(result) => {
                            let elapsed = start.elapsed();
                            println!("{}", result.command_tag());
                            let name = executor.catalog.interner.resolve(match &result {
                                ExecutionResult::DatabaseCreated { name } => *name,
                                _ => unreachable!(),
                            });
                            println!("Database '{}' created successfully.", name);
                            println!("Time: {:?}", elapsed);
                        }
                        Err(e) => {
                            let elapsed = start.elapsed();
                            eprintln!("Execution error: {}", e.format(&executor.catalog.interner));
                            println!("Time: {:?}", elapsed);
                        }
                    }
                }
                Statement::CreateSchema(s) => {
                    let binder = Binder::new(&executor.catalog, executor.session_user);
                    let bound = match binder.bind_create_schema(current_db, s) {
                        Ok(b) => b,
                        Err(e) => {
                            eprintln!("Bind error: {}", e.format(&executor.catalog.interner));
                            continue;
                        }
                    };

                    let start = std::time::Instant::now();
                    match executor.execute_create_schema(current_db, bound) {
                        Ok(result) => {
                            let elapsed = start.elapsed();
                            println!("{}", result.command_tag());
                            let name = executor.catalog.interner.resolve(match &result {
                                ExecutionResult::SchemaCreated { name } => *name,
                                _ => unreachable!(),
                            });
                            println!("Schema '{}' created successfully.", name);
                            println!("Time: {:?}", elapsed);
                        }
                        Err(e) => {
                            let elapsed = start.elapsed();
                            eprintln!("Execution error: {}", e.format(&executor.catalog.interner));
                            println!("Time: {:?}", elapsed);
                        }
                    }
                }
                Statement::CreateTable(s) => {
                    let binder = Binder::new(&executor.catalog, executor.session_user);
                    let bound = match binder.bind_create_table(current_db, public_schema, s) {
                        Ok(b) => b,
                        Err(e) => {
                            eprintln!("Bind error: {}", e.format(&executor.catalog.interner));
                            continue;
                        }
                    };

                    let start = std::time::Instant::now();
                    match executor.execute_create_table(bound) {
                        Ok(result) => {
                            let elapsed = start.elapsed();
                            println!("{}", result.command_tag());
                            let name = executor.catalog.interner.resolve(match &result {
                                ExecutionResult::TableCreated { name } => *name,
                                _ => unreachable!(),
                            });
                            println!("Table '{}' created successfully.", name);
                            println!("Time: {:?}", elapsed);
                        }
                        Err(e) => {
                            let elapsed = start.elapsed();
                            eprintln!("Execution error: {}", e.format(&executor.catalog.interner));
                            println!("Time: {:?}", elapsed);
                        }
                    }
                }
                Statement::Insert(s) => {
                    let binder = Binder::new(&executor.catalog, executor.session_user);
                    let bound = match binder.bind_insert_table(current_db, public_schema, s) {
                        Ok(b) => b,
                        Err(e) => {
                            eprintln!("Bind error: {}", e.format(&executor.catalog.interner));
                            continue;
                        }
                    };

                    let start = std::time::Instant::now();
                    match executor.execute_insert_table(bound) {
                        Ok(result) => {
                            let elapsed = start.elapsed();
                            println!("{}", result.command_tag());
                            let (name, count) = match &result {
                                ExecutionResult::Inserted { name, count } => (*name, *count),
                                _ => unreachable!(),
                            };
                            let table_name = executor.catalog.interner.resolve(name);
                            println!("{} row(s) inserted into '{}'.", count, table_name);
                            println!("Time: {:?}", elapsed);
                        }
                        Err(e) => {
                            let elapsed = start.elapsed();
                            eprintln!("Execution error: {}", e.format(&executor.catalog.interner));
                            println!("Time: {:?}", elapsed);
                        }
                    }
                }
                Statement::Select(s) => {
                    let binder = Binder::new(&executor.catalog, executor.session_user);
                    let bound = match binder.bind_select(current_db, public_schema, s) {
                        Ok(b) => b,
                        Err(e) => {
                            eprintln!("Bind error: {}", e.format(&executor.catalog.interner));
                            continue;
                        }
                    };

                    let start = std::time::Instant::now();
                    match executor.execute_select_table(bound) {
                        Ok(result) => {
                            let elapsed = start.elapsed();
                            println!("{}", result.command_tag());
                            let rows = match result {
                                ExecutionResult::Selected { rows } => rows,
                                _ => unreachable!(),
                            };

                            if rows.is_empty() {
                                println!("(0 rows)");
                            } else {
                                for row in &rows {
                                    let formatted: Vec<String> = row
                                        .iter()
                                        .map(|v| {
                                            format_value(v.clone(), &executor.catalog.interner)
                                        })
                                        .collect();
                                    println!("{}", formatted.join(" | "));
                                }
                                println!(
                                    "({} row{})",
                                    rows.len(),
                                    if rows.len() == 1 { "" } else { "s" }
                                );
                            }
                            println!("Time: {:?}", elapsed);
                        }
                        Err(e) => {
                            let elapsed = start.elapsed();
                            eprintln!("Execution error: {}", e.format(&executor.catalog.interner));
                            println!("Time: {:?}", elapsed);
                        }
                    }
                }
                _ => eprintln!("Unsupported statement"),
            }
        }
    }
}

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
