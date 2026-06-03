use crate::{
    ast::CreateDatabaseStmt,
    lexer::TokenKind,
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
    /// Parses a `CREATE DATABASE` statement.
    ///
    /// Syntax:
    /// ```sql
    /// CREATE DATABASE [IF NOT EXISTS] name
    ///     [OWNER [=] user]
    ///     [ENCODING [=] 'encoding']
    ///     [LOCALE [=] 'locale']
    ///     [TABLESPACE [=] tablespace]
    ///     [CONNECTION LIMIT [=] n]
    /// ```
    ///
    /// The `CREATE DATABASE` keyword is consumed by the caller (`create.rs`).
    /// This function starts at `IF NOT EXISTS` or the database name.
    pub fn parse_create_database(&mut self) -> Result<CreateDatabaseStmt, ParserError> {
        // Parse optional IF NOT EXISTS clause
        let if_not_exists = self.parse_if_not_exist()?;

        // Database name — required
        let name = self.expect_identifier()?;

        // Optional clauses — all can appear in any order
        let mut owner = None;
        let mut encoding = None;
        let mut locale = None;
        let mut tablespace = None;
        let mut connection_limit = None;

        loop {
            match self.current_token().clone() {
                // `OWNER [=] user`
                // Specifies the role that owns the database.
                // The `=` is optional (PostgreSQL allows both forms).
                TokenKind::Owner => {
                    self.advance();
                    self.consume(&TokenKind::Eq);
                    owner = Some(self.expect_identifier()?);
                }

                // `ENCODING [=] 'UTF8'`
                // Sets the character encoding for the database.
                // Value must be a string literal e.g. 'UTF8', 'SQL_ASCII'.
                TokenKind::Encoding => {
                    self.advance();
                    self.consume(&TokenKind::Eq);
                    encoding = Some(self.expect_string_literal()?);
                }

                // `LOCALE [=] 'en_US.UTF-8'`
                // Sets both LC_COLLATE and LC_CTYPE at once.
                // Value must be a string literal.
                TokenKind::Locale => {
                    self.advance();
                    self.consume(&TokenKind::Eq);
                    locale = Some(self.expect_string_literal()?);
                }

                // `TABLESPACE [=] tablespace_name`
                // Specifies the default tablespace for the database.
                TokenKind::Tablespace => {
                    self.advance();
                    self.consume(&TokenKind::Eq);
                    tablespace = Some(self.expect_identifier()?);
                }

                // `CONNECTION LIMIT [=] n`
                // Limits the number of concurrent connections.
                // -1 means no limit.
                TokenKind::Connection => {
                    self.advance();
                    self.expect(TokenKind::Limit)?;
                    self.consume(&TokenKind::Eq);
                    connection_limit = Some(self.expect_int()?);
                }

                // No more clauses — exit the loop
                _ => break,
            }
        }

        Ok(CreateDatabaseStmt {
            name,
            if_not_exists,
            owner,
            encoding,
            locale,
            tablespace,
            connection_limit,
        })
    }
}
