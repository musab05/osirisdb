use crate::lexer::lexer::Lexer;
use crate::lexer::spanned_token::SpannedToken;

pub struct Parser {
    lexer: Lexer,
    current: SpannedToken,
    peek: SpannedToken,
}

impl Parser {
    pub fn new(source: &str) -> Self {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_spanned();
        let peek = lexer.next_spanned();
        Self { lexer, current, peek }
    }
}
