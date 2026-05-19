mod ast;
mod lexer;
mod parser;

use lexer::lexer::Lexer;
use lexer::token::Token;

use crate::parser::parser::Parser;

fn main() {
    // let mut lexer = Lexer::new("SELECT age FROM users;");
    let mut lexer = Lexer::new("SELECT 'hello");
    //     let mut lexer = Lexer::new(
    //         "
    //     SELECT name, age
    //     FROM users
    //     WHERE age >= 18
    //     AND city = 'Mumbai';
    // ",
    //     );

    // loop {
    //     let st = lexer.next_spanned();
    //     println!(
    //         "{:?} @ line {}, col {}, bytes {}..{}",
    //         st.token, st.span.line, st.span.column, st.span.start, st.span.end
    //     );
    //     if st.token == Token::Eof {
    //         break;
    //     }
    // }

    // let mut parser = Parser::new("SELECT id, name FROM users WHERE id = 1");
    // joins
    // let mut parser =
    //     Parser::new("SELECT u.id, o.total FROM users u LEFT JOIN orders o ON u.id = o.user_id");

    // aggregation
    // let mut parser =
    // Parser::new("SELECT city, COUNT(*) FROM users GROUP BY city HAVING COUNT(*) > 10");

    // subquery
    // let mut parser = Parser::new("SELECT * FROM users WHERE id IN (SELECT user_id FROM orders)");

    // CTE
    // let mut parser = Parser::new(
    // "WITH active AS (SELECT * FROM users WHERE active = true) SELECT * FROM active",
    // );

    // set op
    // let mut parser = Parser::new("SELECT id FROM users UNION ALL SELECT id FROM admins");

    let mut parser = Parser::new(
        "
    CREATE TEMPORARY TABLE IF NOT EXISTS public.users (
        id UUID PRIMARY KEY,
        username VARCHAR(255) NOT NULL UNIQUE,
        email CHARACTER VARYING(255) UNIQUE,
        age INT CHECK (age >= 18),
        balance DECIMAL(10, 2) DEFAULT 0.0,
        score DOUBLE PRECISION,
        is_active BOOLEAN DEFAULT true,
        tags TEXT[][],
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
",
    );

    // loop {
    //     let st = parser.next_spanned();
    //     println!(
    //         "{:?} @ line {}, col {}, bytes {}..{}",
    //         st.token, st.span.line, st.span.column, st.span.start, st.span.end
    //     );
    //     if st.token == Token::Eof {
    //         break;
    //     }
    // }

    match parser.parse() {
        Ok(stmts) => {
            for stmt in stmts {
                println!("{:#?}", stmt);
            }
        }
        Err(e) => {
            println!("Parse error: {} at {:?}", e.message, e.span);
        }
    }
}
