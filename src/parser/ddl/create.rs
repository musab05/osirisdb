use crate::{
    ast::Statement,
    lexer::{Modifier, token::TokenKind},
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
    /// Parses a `CREATE` statement.
    ///
    /// Supported CREATE statements include:
    ///
    /// - `CREATE TABLE`
    /// - `CREATE SCHEMA`
    /// - `CREATE INDEX`
    /// - `CREATE VIEW`
    /// - `CREATE MATERIALIZED VIEW`
    /// - `CREATE SEQUENCE`
    ///
    /// CREATE modifiers such as `OR REPLACE`, `TEMPORARY`,
    /// `UNLOGGED`, `UNIQUE`, and `MATERIALIZED` are parsed
    /// and applied when supported by the target object type.
    ///
    /// # Returns
    ///
    /// A [`Statement`] representing the parsed CREATE statement.
    ///
    /// # Errors
    ///
    /// Returns a [`ParserError`] if the token following `CREATE`
    /// does not correspond to a supported CREATE object type or
    /// if any nested CREATE parser encounters invalid syntax.
    pub fn parse_create(&mut self) -> Result<Statement, ParserError> {
        self.consume(&TokenKind::Create);

        let m = self.parse_create_modifiers();

        match self.current_token() {
            TokenKind::Table => {
                self.advance();
                Ok(Statement::CreateTable(
                    self.parse_create_table(m.temporary, m.unlogged)?,
                ))
            }
            TokenKind::Schema => {
                self.advance();
                Ok(Statement::CreateSchema(self.parse_create_schema()?))
            }
            TokenKind::Index => {
                self.advance();
                Ok(Statement::CreateIndex(self.parse_create_index(m.unique)?))
            }
            TokenKind::View => {
                self.advance();
                Ok(Statement::CreateView(self.parse_create_view(
                    m.or_replace,
                    m.temporary,
                    false,
                )?))
            }
            TokenKind::Modifier(Modifier::Materialized) => {
                self.advance();
                self.expect(TokenKind::View)?;
                Ok(Statement::CreateView(self.parse_create_view(
                    m.or_replace,
                    m.temporary,
                    false,
                )?))
            }
            TokenKind::Sequence => {
                self.advance();
                Ok(Statement::CreateSequence(self.parse_create_sequence()?))
            }

            TokenKind::Type => {
                self.advance();
                Ok(Statement::CreateType(self.parse_create_type()?))
            }
            TokenKind::Domain => {
                self.advance();
                Ok(Statement::CreateType(self.parse_create_domain()?))
            }
            TokenKind::Database => {
                self.advance();
                Ok(Statement::CreateDatabase(self.parse_create_database()?))
            }
            TokenKind::Role => {
                self.advance();
                Ok(Statement::CreateRole(self.parse_create_role(false)?))
            }
            TokenKind::User => {
                self.advance();
                Ok(Statement::CreateRole(self.parse_create_role(true)?))
            }
            TokenKind::Tablespace => {
                self.advance();
                Ok(Statement::CreateTablespace(self.parse_create_tablespace()?))
            }
            TokenKind::Extension => {
                self.advance();
                Ok(Statement::CreateExtension(self.parse_create_extension()?))
            }
            TokenKind::Trigger => {
                self.advance();
                Ok(Statement::CreateTrigger(
                    self.parse_create_trigger(m.or_replace)?,
                ))
            }
            TokenKind::Function => {
                self.advance();
                Ok(Statement::CreateFunction(
                    self.parse_create_function(m.or_replace)?,
                ))
            }
            TokenKind::Procedure => {
                self.advance();
                Ok(Statement::CreateProcedure(
                    self.parse_create_procedure(m.or_replace)?,
                ))
            }
            _ => Err(ParserError::new(
                format!(
                    "Expected TABLE/SCHEMA/INDEX/VIEW after CREATE, got {:?}",
                    self.current_token()
                ),
                self.current.span.clone(),
            )),
        }
    }
}
