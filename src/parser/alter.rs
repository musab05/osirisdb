use crate::{
    ast::Statement,
    lexer::Token,
    parser::{parser::Parser, parser_error::ParserError},
};

impl Parser {
    pub fn parse_alter(&mut self) -> Result<Statement, ParserError> {
        self.consume(&Token::Alter);

        match self.current_token().clone() {
            Token::Table => Ok(Statement::AlterTable(self.parse_alter_table()?)),

            _ => Err(ParserError::new(
                format!("Expected TABLE after Alter, got {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }
}
