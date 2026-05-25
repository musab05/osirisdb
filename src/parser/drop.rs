use crate::{
    ast::Statement,
    lexer::token::TokenKind,
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
    pub fn parse_drop(&mut self) -> Result<Statement, ParserError> {
        self.consume(&TokenKind::Drop);
        let m = self.parse_create_modifiers();

        match self.current_token().clone() {
            TokenKind::Table => Ok(Statement::DropTable(self.parse_drop_table(m.temporary)?)),
            _ => Err(ParserError::new(
                format!("Expected TABLE after DROP, got {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }
}
