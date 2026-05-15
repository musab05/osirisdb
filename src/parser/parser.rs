use crate::lexer::lexer::Lexer;
use crate::lexer::spanned_token::SpannedToken;
use crate::lexer::token::Token;
use crate::parser::parser_error::ParserError;

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
        Self {
            lexer,
            current,
            peek,
        }
    }

    pub fn advance(&mut self) {
        self.current = self.peek.clone();
        self.peek = self.lexer.next_spanned();
    }

    pub fn current_token(&self) -> &Token {
        &self.current.token
    }

    pub fn peek_token(&self) -> &Token {
        &self.peek.token
    }

    pub fn consume(&mut self, expected: &Token) -> bool {
        if self.current_token() == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParserError> {
        if self.current.token == expected {
            self.advance();
            Ok(())
        } else {
            Err(ParserError::new(
                format!("Expected {:?}, found {:?}", expected, self.current.token),
                self.current.span.clone(),
            ))
        }
    }
}
