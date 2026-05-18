use crate::{
    ast::Statement, lexer::token::Token, parser::{parser::Parser, parser_error::ParserError}
};

impl Parser {
    pub fn parse(&mut self) -> Result<Vec<Statement>, ParserError> {
        let mut stmts = vec![];

        while !self.is_at_end() {
            if self.consume(&Token::Semicolon) {
                continue;
            }
            stmts.push(self.parse_statement()?);
        }

        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Statement, ParserError> {
        let stmt = match self.current_token() {
            Token::Select | Token::With => Statement::Select(self.parse_select()?),
            Token::Create => Statement::CreateTable(self.parse_create()?),

            _ => {
                return Err(ParserError::new(
                    format!("Unexpected token {:?}", self.current_token()),
                    self.current.span.clone(),
                ));
            }
        };

        self.consume(&Token::Semicolon);

        Ok(stmt)
    }
}
