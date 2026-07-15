use crate::{catalog::CatalogManager, parser::Parser};

pub fn sql() {
    let mut sql = String::new();

    println!("Enter your command");

    loop {
        std::io::stdin()
            .read_line(&mut sql)
            .expect("Failed to read line");

        let mut parser = Parser::new(&sql);
        let stmts = match parser.parse() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Parse error: {}", e);
                continue;
            }
        };

        let interner = parser.interner;
        let catalog = CatalogManager::new(interner);

        let session_user = catalog.interner.intern("postgres");
        todo!()
    }
}
