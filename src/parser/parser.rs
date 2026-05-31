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

    pub fn current_token(&self) -> &TokenKind {
        &self.current.kind
    }

    pub fn current_span(&self) -> &Span {
        &self.current.span
    }

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

    pub fn is_at_end(&self) -> bool {
        *self.current_token() == TokenKind::Eof
    }

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
}
