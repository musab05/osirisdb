use crate::lexer::lexer::Lexer;
use crate::lexer::spanned_token::Span;
use crate::lexer::token::{Token, TokenKind};
use crate::parser::parser_error::ParserError;

pub struct Parser<'a> {
    pub source: &'a str,
    pub lexer: Lexer<'a>,
    pub current: Token,
    pub peek: Token,
}

impl<'a> Parser<'a> {
    pub fn new(source: &'a str) -> Self {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token();
        let peek = lexer.next_token();
        Self {
            source,
            lexer,
            current,
            peek,
        }
    }

    pub fn advance(&mut self) {
        self.current = self.peek.clone();
        self.peek = self.lexer.next_token();
    }

    pub fn current_token(&self) -> &TokenKind {
        &self.current.kind
    }

    pub fn current_span(&self) -> &Span {
        &self.current.span
    }

    pub fn peek_token(&self) -> &TokenKind {
        &self.peek.kind
    }

    pub fn consume(&mut self, expected: &TokenKind) -> bool {
        if self.current_token() == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn is_at_end(&self) -> bool {
        *self.current_token() == TokenKind::Eof
    }

    pub fn peek_is(&self, token: &TokenKind) -> bool {
        self.peek_token() == token
    }

    pub fn expect(&mut self, expected: TokenKind) -> Result<(), ParserError> {
        if self.current.kind == expected {
            self.advance();
            Ok(())
        } else {
            Err(ParserError::new(
                format!("Expected {:?}, found {:?}", expected, self.current.kind),
                self.current.span.clone(),
            ))
        }
    }

    pub fn expect_identifier(&mut self) -> Result<String, ParserError> {
        match self.current.kind {
            TokenKind::Ident => {
                let s = self.source[self.current.span.start..self.current.span.end].to_string();
                self.advance();
                Ok(s)
            }
            TokenKind::QuotedIdent => {
                let s =
                    self.source[self.current.span.start + 1..self.current.span.end - 1].to_string();
                self.advance();
                Ok(s)
            }
            _ => Err(ParserError::new(
                format!("Expected identifier, found {:?}", self.current.kind),
                self.current.span.clone(),
            )),
        }
    }
}
