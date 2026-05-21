use crate::lexer::lexer::Lexer;
use crate::lexer::spanned_token::{Span, SpannedToken};
use crate::lexer::token::Token;
use crate::parser::parser_error::ParserError;

pub struct Parser {
    pub lexer: Lexer,
    pub current: SpannedToken,
    pub peek: SpannedToken,
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

    pub fn current_span(&self) -> &Span {
        &self.current.span
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

    pub fn is_at_end(&self) -> bool {
        *self.current_token() == Token::Eof
    }

    pub fn peek_is(&self, token: &Token) -> bool {
        self.peek_token() == token
    }

    pub fn expect(&mut self, expected: Token) -> Result<(), ParserError> {
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

    pub fn expect_identifier(&mut self) -> Result<String, ParserError> {
        match self.current.token.clone() {
            Token::Ident(name) | Token::QuotedIdent(name) => {
                self.advance();
                Ok(name)
            }
            _ => Err(ParserError::new(
                format!("Expected identifier, found {:?}", self.current.token),
                self.current.span.clone(),
            )),
        }
    }
    
}
