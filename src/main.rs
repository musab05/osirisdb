mod lexer;

use lexer::lexer::Lexer;
use lexer::token::Token;

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

    loop {
        let st = lexer.next_spanned();
        println!(
            "{:?} @ line {}, col {}, bytes {}..{}",
            st.token, st.span.line, st.span.column, st.span.start, st.span.end
        );
        if st.token == Token::Eof {
            break;
        }
    }
}
