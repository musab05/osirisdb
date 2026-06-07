use crate::common::interner::Interner;
use crate::common::symbol::Symbol;
use crate::lexer::lexer::Lexer;
use crate::lexer::spanned_token::Span;
use crate::lexer::token::{Token, TokenKind};
use crate::parser::parser_error::ParserError;

/// A recursive-descent and Pratt parser for SQL queries.
///
/// # Architecture
///
/// The parser operates on a token stream produced by [`Lexer`] and builds
/// an Abstract Syntax Tree (AST) representing the SQL query structure.
///
/// It uses a two-token lookahead (`current` and `peek`) which is sufficient
/// for all SQL constructs without backtracking.
///
/// # Parsing strategy
///
/// - **Recursive descent** for statements, clauses, and declarations
/// - **Pratt parsing** for expressions — handles operator precedence and
///   associativity cleanly without a grammar table
///
/// # String interning
///
/// All identifier and string literal names are interned via [`Interner`]
/// and stored as [`Symbol`]s throughout the AST. This means:
/// - No redundant heap allocations for repeated identifiers
/// - O(1) name comparison everywhere downstream (binder, optimizer, executor)
/// - The interner is owned by the parser and passed to subsequent pipeline
///   stages after parsing completes
///
/// # Lifetime
///
/// `'a` is the lifetime of the source SQL string. The parser borrows the
/// source to slice identifier spans directly — no copying until `intern()`.
///

pub struct Parser<'a> {
    /// The original source SQL string.
    ///
    /// Held as a reference so identifier spans can be sliced directly
    /// from the source without copying: `&self.source[span.start..span.end]`.
    /// The slice is only copied once — when interned.
    pub source: &'a str,

    /// The underlying lexical analyzer that produces tokens on demand.
    ///
    /// The lexer is consumed token by token via [`advance`]. It is not
    /// reset or rewound — parsing is strictly forward-only.
    pub lexer: Lexer<'a>,

    /// The current token being examined.
    ///
    /// All parse decisions are made based on this token. After a decision
    /// is made, [`advance`] moves this forward.
    pub current: Token,

    /// The next token after `current`.
    ///
    /// Used for two-token lookahead decisions such as distinguishing
    /// `NOT NULL` from `NOT IN`, or `IS NULL` from `IS NOT NULL`,
    /// without consuming `current`.
    pub peek: Token,

    /// The string interner for this parse session.
    ///
    /// Every identifier and string literal is interned here and stored
    /// as a [`Symbol`] in the AST. After parsing completes, the caller
    /// takes ownership of the interner and passes it to the binder:
    ///

    pub interner: Interner,
}

impl<'a> Parser<'a> {
    /// Creates a new `Parser` for the given SQL source string.
    ///
    /// Pre-fetches the first two tokens into `current` and `peek` so the
    /// parser is immediately ready to make decisions without calling advance.
    ///

    pub fn new(source: &'a str) -> Self {
        let mut lexer = Lexer::new(source);
        let current = lexer.next_token();
        let peek = lexer.next_token();
        Self {
            source,
            lexer,
            current,
            peek,
            interner: Interner::new(),
        }
    }

    /// Advances the parser by one token.
    ///
    /// `peek` becomes `current`, and a new token is fetched from the lexer
    /// into `peek`. This is the only way tokens are consumed — every parse
    /// function calls `advance` after it is done with a token.
    pub fn advance(&mut self) {
        self.current = self.peek.clone();
        self.peek = self.lexer.next_token();
    }

    /// Returns a reference to the kind of the current token.
    ///
    /// Used for all branching decisions in the parser without consuming
    /// the token. To consume after inspection, call [`advance`] or use
    /// [`consume`] / [`expect`].
    pub fn current_token(&self) -> &TokenKind {
        &self.current.kind
    }

    /// Returns the span of the current token.
    ///
    /// The span holds the byte offsets (`start`, `end`) and source position
    /// (`line`, `column`) of the current token within the source string.
    /// Used for attaching location info to [`ParserError`]s.
    pub fn current_span(&self) -> &Span {
        &self.current.span
    }

    /// Returns a reference to the kind of the next token without consuming it.
    ///
    /// Enables two-token lookahead for ambiguous constructs. For example,
    /// after seeing `NOT`, peek tells us whether this is `NOT NULL`,
    /// `NOT IN`, or `NOT EXISTS` without committing to any branch.
    pub fn peek_token(&self) -> &TokenKind {
        &self.peek.kind
    }

    /// Consumes the current token if it matches `expected`.
    ///
    /// Returns `true` and advances if the token matches.
    /// Returns `false` and does nothing if it does not.
    ///
    /// Use this for **optional** tokens where absence is valid:

    ///
    /// For **required** tokens where absence is an error, use [`expect`].
    /// Never use `consume` after a `match` arm on the same token — use
    /// [`advance`] instead since the match already confirmed the token.
    pub fn consume(&mut self, expected: &TokenKind) -> bool {
        if self.current_token() == expected {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Returns `true` if the parser has reached the end of the token stream.
    ///
    /// The end is signalled by [`TokenKind::Eof`] which the lexer emits
    /// after exhausting all input. The parser checks this in the top-level
    /// statement loop to know when to stop.
    pub fn is_at_end(&self) -> bool {
        *self.current_token() == TokenKind::Eof
    }

    /// Returns `true` if the next (peek) token matches `token`.
    ///
    /// Does not consume any tokens. Used for two-token lookahead decisions
    /// where you need to see beyond `current` before committing.
    ///

    pub fn peek_is(&self, token: &TokenKind) -> bool {
        self.peek_token() == token
    }

    /// Consumes the current token if it matches `expected`, or returns an error.
    ///
    /// Use this for **required** tokens where absence is a syntax error:

    ///
    /// For **optional** tokens, use [`consume`].
    /// For tokens already confirmed by a `match` arm, use [`advance`].
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

    /// Consumes the current token expecting it to be an identifier or quoted
    /// identifier, interns the name, and returns its [`Symbol`].
    ///
    /// # Interning behavior
    ///
    /// The identifier text is sliced directly from `self.source` using the
    /// token's span — no allocation if the string has been seen before.
    /// For quoted identifiers (`"name"`), the surrounding quotes are stripped
    /// before interning so `"users"` and `users` intern to the same symbol.
    ///
    /// # Errors
    ///
    /// Returns [`ParserError`] if the current token is not an identifier.
    ///

    pub fn expect_identifier(&mut self) -> Result<Symbol, ParserError> {
        match self.current.kind {
            // Unquoted identifier: SELECT, users, my_table etc.
            // Slice directly from source — zero copy until intern()
            TokenKind::Ident => {
                let s = &self.source[self.current.span.start..self.current.span.end];
                let sym = self.interner.intern(s);
                self.advance();
                Ok(sym)
            }
            // Quoted identifier: "My Table", "select" (reserved word used as name)
            // Strip the surrounding double quotes before interning so
            // `"users"` and `users` resolve to the same symbol.
            TokenKind::QuotedIdent => {
                let s = &self.source[self.current.span.start + 1..self.current.span.end - 1];
                let sym = self.interner.intern(s);
                self.advance();
                Ok(sym)
            }
            _ => Err(ParserError::new(
                format!("Expected identifier, found {:?}", self.current.kind),
                self.current.span.clone(),
            )),
        }
    }

    /// Consumes the current token expecting it to be a string literal,
    /// interns the content, and returns its [`Symbol`].
    ///
    /// # Quote stripping
    ///
    /// String literals in SQL are surrounded by single quotes: `'hello'`.
    /// The span covers the entire literal including quotes, so we slice
    /// `start + 1..end - 1` to get the content without the delimiters.
    ///
    /// # Interning behavior
    ///
    /// String literal contents are interned the same way as identifiers.
    /// This is particularly useful for repeated values like encoding names,
    /// locale strings, and role passwords in DDL statements.
    ///
    /// # Errors
    ///
    /// Returns [`ParserError`] if the current token is not a string literal.
    ///

    pub fn expect_string_literal(&mut self) -> Result<Symbol, ParserError> {
        match self.current_token() {
            TokenKind::StringLit => {
                // Strip surrounding single quotes: 'value' → value
                let s = &self.source[self.current.span.start + 1..self.current.span.end - 1];
                let sym = self.interner.intern(s);
                self.advance();
                Ok(sym)
            }
            _ => Err(ParserError::new(
                format!("Expected string literal, found {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }

    /// Expects the current token to be a positive integer literal and returns
    /// its value as `u64`.
    ///
    /// Used for contexts where only non-negative integers are valid:
    /// `LIMIT`, `OFFSET`, `FETCH NEXT n ROWS`, `CONNECTION LIMIT` etc.
    ///
    /// For signed integers (e.g. `MINVALUE`, `START WITH` in sequences),
    /// use [`expect_int`] instead which handles a leading `-` sign.
    ///
    /// # Errors
    ///
    /// Returns [`ParserError`] if the current token is not a non-negative
    /// integer literal.
    pub fn expect_int_literal(&mut self) -> Result<u64, ParserError> {
        match self.current_token().clone() {
            TokenKind::IntLit(n) if n >= 0 => {
                self.advance();
                Ok(n as u64)
            }
            _ => Err(ParserError::new(
                format!(
                    "Expected positive integer, found {:?}",
                    self.current_token()
                ),
                self.current.span.clone(),
            )),
        }
    }

    /// Expects the current token to be an integer, handling an optional
    /// leading `-` sign. Returns a signed `i64`.
    ///
    /// Used for contexts where negative integers are valid:
    /// `MINVALUE`, `MAXVALUE`, `START WITH`, `INCREMENT BY` in sequences.
    ///
    /// For unsigned-only contexts, use [`expect_int_literal`].
    ///
    /// # Errors
    ///
    /// Returns [`ParserError`] if the current token is not an integer
    /// or a `-` followed by an integer.
    pub fn expect_int(&mut self) -> Result<i64, ParserError> {
        match self.current_token().clone() {
            TokenKind::IntLit(n) => {
                self.advance();
                Ok(n)
            }
            TokenKind::Minus => {
                self.advance();
                match self.current_token().clone() {
                    TokenKind::IntLit(n) => {
                        self.advance();
                        Ok(-n)
                    }
                    _ => Err(ParserError::new(
                        "Expected integer after -",
                        self.current.span.clone(),
                    )),
                }
            }
            _ => Err(ParserError::new(
                format!("Expected integer, got {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }

    /// Expects the current token to be a numeric literal (integer or float)
    /// and returns it as `f64`.
    ///
    /// Used for planner cost estimates: `COST n`, `ROWS n` in
    /// `CREATE FUNCTION` where fractional values are valid.
    ///
    /// # Errors
    ///
    /// Returns [`ParserError`] if the current token is not a numeric literal.
    pub fn expect_float(&mut self) -> Result<f64, ParserError> {
        match self.current_token().clone() {
            TokenKind::FloatLit(f) => {
                self.advance();
                Ok(f)
            }
            TokenKind::IntLit(n) => {
                self.advance();
                Ok(n as f64)
            }
            TokenKind::Minus => {
                self.advance();
                match self.current_token().clone() {
                    TokenKind::FloatLit(f) => {
                        self.advance();
                        Ok(-f)
                    }
                    TokenKind::IntLit(n) => {
                        self.advance();
                        Ok(-(n as f64))
                    }
                    _ => Err(ParserError::new(
                        "Expected number after -",
                        self.current.span.clone(),
                    )),
                }
            }
            _ => Err(ParserError::new(
                format!("Expected number, got {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }

    /// Parses a qualified (dot-separated) name and returns it as a
    /// `Vec<Symbol>`.
    ///
    /// Handles simple and schema-qualified names:
    /// ```sql
    /// users                    → vec![Symbol("users")]
    /// public.users             → vec![Symbol("public"), Symbol("users")]
    /// mydb.public.users        → vec![Symbol("mydb"), Symbol("public"), Symbol("users")]
    /// ```
    ///
    /// Each part is interned individually so repeated schema names
    /// (e.g. `public`) cost nothing after the first occurrence.
    ///
    /// # Errors
    ///
    /// Returns [`ParserError`] if the first token is not an identifier.
    pub fn parse_qualified_name(&mut self) -> Result<Vec<Symbol>, ParserError> {
        let mut parts = vec![self.expect_identifier()?];
        while self.consume(&TokenKind::Dot) {
            parts.push(self.expect_identifier()?);
        }
        Ok(parts)
    }

    /// Parses an optional `IF NOT EXISTS` clause.
    ///
    /// Returns `true` if the clause was present and consumed,
    /// `false` if it was absent. Used by all `CREATE` statement parsers
    /// that support this clause.
    ///
    /// # Errors
    ///
    /// Returns [`ParserError`] if `IF` is present but `NOT` or `EXISTS`
    /// does not follow — malformed `IF NOT EXISTS` is a syntax error.
    pub fn parse_if_not_exist(&mut self) -> Result<bool, ParserError> {
        if self.consume(&TokenKind::If) {
            self.expect(TokenKind::Not)?;
            self.expect(TokenKind::Exists)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
