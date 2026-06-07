use crate::{
    ast::CreateExtensionStmt,
    lexer::TokenKind,
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
    /// Parses a `CREATE EXTENSION` statement.
    ///
    /// Syntax:
    /// ```sql
    /// CREATE EXTENSION [IF NOT EXISTS] name
    ///     [SCHEMA [=] schema_name]
    ///     [VERSION [=] 'version']
    ///     [CASCADE]
    /// ```
    ///
    /// `CASCADE` automatically installs any required dependency extensions.
    /// All clauses except the name are optional and can appear in any order.
    /// The `EXTENSION` keyword is consumed by the caller (`create.rs`).
    pub fn parse_create_extension(&mut self) -> Result<CreateExtensionStmt, ParserError> {
        // Optional IF NOT EXISTS clause
        let if_not_exists = self.parse_if_not_exist()?;

        // Extension name — required
        let name = self.expect_identifier()?;

        let mut schema = None;
        let mut version = None;
        let mut cascade = false;

        // All remaining clauses are optional and order-independent
        loop {
            match self.current_token().clone() {
                // SCHEMA [=] schema_name — installs extension into this schema.
                // Accepts both `SCHEMA public` and `SCHEMA = public`.
                TokenKind::Schema => {
                    self.advance();
                    self.consume(&TokenKind::Eq);
                    schema = Some(self.expect_identifier()?);
                }

                // VERSION [=] 'version' — installs a specific version.
                // Accepts both `VERSION '1.3'` and `VERSION = '1.3'`.
                TokenKind::Version => {
                    self.advance();
                    self.consume(&TokenKind::Eq);
                    version = Some(self.expect_string_literal()?);
                }

                // CASCADE — automatically installs any extensions
                // that this extension depends on.
                TokenKind::Cascade => {
                    self.advance();
                    cascade = true;
                }

                // No more recognized clauses — stop parsing
                _ => break,
            }
        }

        Ok(CreateExtensionStmt {
            name,
            if_not_exists,
            schema,
            version,
            cascade,
        })
    }
}
