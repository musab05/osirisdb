use crate::{
    ast::Statement,
    lexer::{Modifier, token::TokenKind},
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
    pub fn parse_create(&mut self) -> Result<Statement, ParserError> {
        self.consume(&TokenKind::Create);

        let m = self.parse_create_modifiers();

        match self.current_token() {
            TokenKind::Table => {
                let stmt = self.parse_create_table(m.temporary, m.unlogged)?;
                Ok(Statement::CreateTable(stmt))
            }
            TokenKind::Schema => Ok(Statement::CreateSchema(self.parse_create_schema()?)),
            TokenKind::Index => Ok(Statement::CreateIndex(self.parse_create_index(m.unique)?)),
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
            TokenKind::Sequence => Ok(Statement::CreateSequence(self.parse_create_sequence()?)),
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
