use crate::lexer::lexer::Lexer;
use crate::lexer::spanned_token::Span;
use crate::lexer::token::{Token, TokenKind};
use crate::parser::parser_error::ParserError;

/// A recursive-descent and Pratt parser for SQL queries.
///
/// Implements token navigation helpers and lookahead caching (`current` and `peek`).
pub struct Parser<'a> {
    /// The original source SQL query string.
    pub source: &'a str,
    /// The underlying lexical analyzer.
    pub lexer: Lexer<'a>,
    /// The current lookahead token.
    pub current: Token,
    /// The next lookahead token.
    pub peek: Token,
}

impl<'a> Parser<'a> {
    /// Creates a new `Parser` for the given SQL string, pre-fetching the first two tokens.
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

    /// Advances the parser by one token, updating both `current` and `peek`.
    pub fn advance(&mut self) {
        self.current = self.peek.clone();
        self.peek = self.lexer.next_token();
    }

    /// Executes parsing or lookup for the `current_token` operation.
    pub fn current_token(&self) -> &TokenKind {
        &self.current.kind
    }

    /// Executes parsing or lookup for the `current_span` operation.
    pub fn current_span(&self) -> &Span {
        &self.current.span
    }

    /// Executes parsing or lookup for the `peek_token` operation.
    pub fn peek_token(&self) -> &TokenKind {
        &self.peek.kind
    }

    /// Tries to consume the next token if it matches `expected`.
    /// Returns `true` if consumed, otherwise `false`.
    pub fn consume(&mut self, expected: &TokenKind) -> bool {
        if self.current_token() == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Executes parsing or lookup for the `is_at_end` operation.
    pub fn is_at_end(&self) -> bool {
        *self.current_token() == TokenKind::Eof
    }

    /// Executes parsing or lookup for the `peek_is` operation.
    pub fn peek_is(&self, token: &TokenKind) -> bool {
        self.peek_token() == token
    }

    /// Consumes the next token if it matches `expected`, or returns a [`ParserError`].
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

    /// Consumes the next token expecting it to be an identifier or quoted identifier,
    /// returning its string contents. Strips quotes for quoted identifiers.
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

    /// Expects the current token to be a string literal and returns its contents.
    /// Strips the surrounding quotes from the span.
    pub fn expect_string_literal(&mut self) -> Result<String, ParserError> {
        match self.current_token() {
            TokenKind::StringLit => {
                let s =
                    self.source[self.current.span.start + 1..self.current.span.end - 1].to_string();
                self.advance();
                Ok(s)
            }
            _ => Err(ParserError::new(
                format!("Expected string literal, found {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }
}
