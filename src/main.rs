use rust_sql::{
    binder::Binder,
    catalog::CatalogManager,
    executor::Executor,
    parser::Parser,
    storage::Storage,
    ast::Statement,
};

fn main() {
    let sql = "CREATE DATABASE mydb OWNER postgres ENCODING 'UTF8' CONNECTION LIMIT 100;";

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
                            executor.catalog.get_database(
                                executor.catalog.interner.get("mydb").unwrap()
                            ).unwrap().name
                        );
                        println!("Database '{}' created successfully.", name);
                    }
                    Err(e) => eprintln!("Execution error: {}", e),
                }
            }
            _ => eprintln!("Unsupported statement"),
        }
    }
}