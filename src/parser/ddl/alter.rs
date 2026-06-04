use crate::{
    ast::Statement,
    lexer::TokenKind,
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
/// Executes parsing or lookup for the `parse_alter` operation.
    pub fn parse_alter(&mut self) -> Result<Statement, ParserError> {
        self.consume(&TokenKind::Alter);

        match self.current_token().clone() {
            TokenKind::Table => Ok(Statement::AlterTable(self.parse_alter_table()?)),

            _ => Err(ParserError::new(
                format!("Expected TABLE after Alter, got {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }
}
