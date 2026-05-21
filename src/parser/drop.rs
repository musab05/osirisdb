use crate::{
    ast::Statement,
    lexer::token::Token,
    parser::{parser::Parser, parser_error::ParserError},
};

impl Parser {
    pub fn parse_drop(&mut self) -> Result<Statement, ParserError> {
        self.consume(&Token::Drop);
        let m = self.parse_create_modifiers();

        match self.current_token().clone() {
            Token::Table => Ok(Statement::DropTable(self.parse_drop_table(m.temporary)?)),
            _ => Err(ParserError::new(
                format!("Expected TABLE after DROP, got {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }
}
