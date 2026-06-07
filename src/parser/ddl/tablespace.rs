use crate::{
    ast::{CreateTablespaceStmt, SqlOption},
    lexer::TokenKind,
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
    /// Parses a `CREATE TABLESPACE` statement.
    ///
    /// Syntax:
    /// ```sql
    /// CREATE TABLESPACE name
    ///     [OWNER [=] user]
    ///     LOCATION [=] '/path/to/dir'
    ///     [WITH (option = value, ...)]
    /// ```
    ///
    /// `LOCATION` is required — all other clauses are optional.
    /// The `TABLESPACE` keyword is consumed by the caller (`create.rs`).
    pub fn parse_create_tablespace(&mut self) -> Result<CreateTablespaceStmt, ParserError> {
        // Tablespace name — required
        let name = self.expect_identifier()?;

        // Optional OWNER clause — specifies who owns the tablespace.
        // Accepts both `OWNER alice` and `OWNER = alice`.
        let owner = if self.consume(&TokenKind::Owner) {
            self.consume(&TokenKind::Eq);
            Some(self.expect_identifier()?)
        } else {
            None
        };

        // Required LOCATION clause — absolute path on the filesystem
        // where the tablespace data will be stored.
        // Accepts both `LOCATION '/path'` and `LOCATION = '/path'`.
        self.expect(TokenKind::Location)?;
        self.consume(&TokenKind::Eq);
        let location = self.expect_string_literal()?;

        // Optional WITH clause — key/value storage parameters.
        // Example: WITH (seq_page_cost = 1.0, random_page_cost = 2.0)
        let options: Vec<SqlOption> = if self.consume(&TokenKind::With) {
            self.parse_options_list()?
        } else {
            vec![]
        };

        Ok(CreateTablespaceStmt {
            name,
            owner,
            location,
            options,
        })
    }
}
