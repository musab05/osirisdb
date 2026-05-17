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
    WITH 
        active_users AS (
            SELECT id, name, city
            FROM users
            WHERE active = true
        ),
        order_counts AS (
            SELECT user_id, COUNT(*) AS total_orders
            FROM orders
            GROUP BY user_id
            HAVING COUNT(*) > 5
        )
    SELECT DISTINCT ON (u.city)
        u.id,
        u.name,
        u.city,
        o.total_orders,
        CASE 
            WHEN o.total_orders > 100 THEN 'platinum'
            WHEN o.total_orders > 50  THEN 'gold'
            ELSE 'silver'
        END AS tier,
        CAST(u.id AS bigint),
        u.created_at::timestamp,
        COUNT(*) AS user_count,
        SUM(o.total_orders) AS sum_orders
    FROM active_users u
    LEFT JOIN order_counts o ON u.id = o.user_id
    INNER JOIN cities c ON u.city = c.name
    WHERE
        u.id IN (SELECT user_id FROM vip_list)
        AND u.city = 'Mumbai'
        AND o.total_orders BETWEEN 10 AND 200
        AND u.name LIKE 'A%'
        AND u.deleted_at IS NULL
        AND EXISTS (SELECT 1 FROM subscriptions s WHERE s.user_id = u.id)
        AND NOT u.banned
    GROUP BY u.city, u.id, u.name, o.total_orders
    HAVING COUNT(*) > 1
    ORDER BY o.total_orders DESC, u.name ASC
    LIMIT 50
    OFFSET 10
    UNION ALL
    SELECT DISTINCT
        u.id,
        u.name,
        u.city,
        0,
        'bronze',
        CAST(u.id AS bigint),
        u.created_at::timestamp,
        1,
        0
    FROM users u
    WHERE u.active = false
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
