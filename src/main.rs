use rust_sql::parser::Parser;

/// Demo entry point for the `rust_sql` parser binary.
/// Parses a complex SQL script and prints the resulting Abstract Syntax Tree (AST).
fn main() {
    println!("=== rust_sql Parser Demo ===");

    // Example of a complex CREATE TABLE statement demonstrating features like
    // inherits, partitioning, column constraints, defaults, and generated columns.
    let create_table_sql = "
        CREATE TEMPORARY TABLE IF NOT EXISTS public.users (
            id UUID PRIMARY KEY,
            username VARCHAR(255) NOT NULL UNIQUE,
            email CHARACTER VARYING(255) UNIQUE,
            age INT CHECK (age >= 18),
            balance DECIMAL(10, 2) DEFAULT 0.0,
            score DOUBLE PRECISION,
            is_active BOOLEAN DEFAULT true,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            virtual_col INT GENERATED ALWAYS AS (age * 2) STORED,
            org_id UUID REFERENCES organizations(id) ON DELETE CASCADE,
            CONSTRAINT email_check CHECK (email LIKE '%@%'),
            CONSTRAINT fk_org FOREIGN KEY (org_id) REFERENCES organizations(id) ON UPDATE SET NULL
        )
        INHERITS (base_users)
        PARTITION BY RANGE (created_at)
        WITH (fillfactor = 70)
        TABLESPACE fastspace
        ON COMMIT PRESERVE ROWS;
    ";

    // Example of a SELECT query showing CTEs, joins, filters, ordering and limit.
    let select_sql = "
        WITH active_users AS (
            SELECT u.id, u.username
            FROM public.users u
            WHERE u.is_active = true
        )
        SELECT au.id, au.username, o.amount
        FROM active_users au
        JOIN orders o ON au.id = o.user_id
        ORDER BY o.amount DESC, au.username ASC
        LIMIT 10;
    ";

    let queries = [create_table_sql, select_sql];

    for (i, sql) in queries.iter().enumerate() {
        println!("\n--- Query {} ---", i + 1);
        println!("Input SQL:\n{}", sql.trim());

        let mut parser = Parser::new(sql);
        match parser.parse() {
            Ok(statements) => {
                println!("\nParsed AST successfully:");
                for stmt in statements {
                    println!("{:#?}", stmt);
                }
            }
            Err(e) => {
                eprintln!("\nParse error: {} at {:?}", e.message, e.span);
            }
        }
    }
}
