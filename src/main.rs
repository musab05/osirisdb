use osirisdb::{
    ast::Statement,
    binder::Binder,
    catalog::CatalogManager,
    executor::{ExecutionResult, Executor},
    parser::Parser,
    storage::Storage,
};

fn main() {
    let sql = "CREATE DATABASE mydb OWNER postgres ENCODING 'UTF8' CONNECTION LIMIT 100; \
                CREATE SCHEMA myschema; \
                CREATE TABLE users (id INT PRIMARY KEY, name VARCHAR(255) NOT NULL, email VARCHAR(255) UNIQUE);";

    // ── 1. Parse ──────────────────────────────────────────────────────────────
    let mut parser = Parser::new(sql);
    let stmts = match parser.parse() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            return;
        }
    };
    let interner = parser.interner;

    // ── 2. Setup catalog + storage + executor ─────────────────────────────────
    let mut catalog = CatalogManager::new(interner);
    let session_user = catalog.interner.intern("postgres");

    // NOTE: there is no connection/session layer yet, so "current database"
    // and "default schema" are hardcoded here. In a real engine these come
    // from the connection (e.g. `\c mydb`) and `search_path`, and would be
    // tracked per-session.
    let current_db = catalog.interner.intern("mydb");
    let public_schema = catalog.interner.intern("public");

    let storage = match Storage::new_or_create("./data") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Storage error: {}", e);
            return;
        }
    };

    let mut executor = Executor::new(catalog, session_user, storage);

    // ── 3. Bind + execute each statement ──────────────────────────────────────
    for stmt in stmts {
        match stmt {
            Statement::CreateDatabase(s) => {
                let binder = Binder::new(&executor.catalog, executor.session_user);
                let bound = match binder.bind_create_database(s) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("Bind error: {}", e);
                        return;
                    }
                };

                match executor.execute_create_database(bound) {
                    Ok(result) => {
                        println!("{}", result.command_tag());
                        // show what was created
                        let name = executor.catalog.interner.resolve(
                            executor
                                .catalog
                                .get_database(executor.catalog.interner.get("mydb").unwrap())
                                .unwrap()
                                .name,
                        );
                        println!("Database '{}' created successfully.", name);
                    }
                    Err(e) => eprintln!("Execution error: {}", e),
                }
            }
            Statement::CreateSchema(s) => {
                let binder = Binder::new(&executor.catalog, executor.session_user);
                let bound = match binder.bind_create_schema(current_db, s) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("Bind error: {}", e);
                        return;
                    }
                };

                match executor.execute_create_schema(current_db, bound) {
                    Ok(result) => {
                        println!("{}", result.command_tag());
                        // show what was created
                        let name = executor.catalog.interner.resolve(match &result {
                            ExecutionResult::SchemaCreated { name } => *name,
                            _ => unreachable!("execute_create_schema only returns SchemaCreated"),
                        });
                        println!("Schema '{}' created successfully.", name);
                    }
                    Err(e) => eprintln!("Execution error: {}", e),
                }
            }
            Statement::CreateTable(s) => {
                let binder = Binder::new(&executor.catalog, executor.session_user);
                let bound = match binder.bind_create_table(current_db, public_schema, s) {
                    Ok(b) => b,
                    Err(e) => {
                        eprintln!("Bind error: {}", e);
                        return;
                    }
                };

                match executor.execute_create_table(bound) {
                    Ok(result) => {
                        println!("{}", result.command_tag());
                        // show what was created
                        let name = executor.catalog.interner.resolve(match &result {
                            ExecutionResult::TableCreated { name } => *name,
                            _ => unreachable!("execute_create_table only returns TableCreated"),
                        });
                        println!("Table '{}' created successfully.", name);
                    }
                    Err(e) => eprintln!("Execution error: {}", e),
                }
            }
            _ => eprintln!("Unsupported statement"),
        }
    }
}
