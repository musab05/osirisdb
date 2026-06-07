use rust_sql::parser::parser::Parser;

fn main() {
    let sql = "SELECT id, name FROM users WHERE id = 1;";
    let mut parser = Parser::new(sql);
    match parser.parse() {
        Ok(stmts) => println!("Parsed {} statement(s)", stmts.len()),
        Err(e) => eprintln!("Error: {} at {:?}", e.message, e.span),
    }
}