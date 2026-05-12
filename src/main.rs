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
        let token = lexer.next_token();

        println!("{:?}", token);

        if token == Token::Eof {
            break;
        }
    }
}
