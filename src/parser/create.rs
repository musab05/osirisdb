use crate::{
    ast::Statement,
    lexer::token::TokenKind,
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
            _ => Err(ParserError::new(
                format!(
                    "Expected TABLE after CREATE, got {:?}",
                    self.current_token()
                ),
                self.current.span.clone(),
            )),
        }
    }
}
