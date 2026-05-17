use crate::{
    ast::stmt::Stmt,
    lexer::token::Token,
    parser::{parser::Parser, parser_error::ParserError},
};

impl Parser {
    pub fn parse(&mut self) -> Result<Vec<Stmt>, ParserError> {
        let mut stmts = vec![];

        while !self.is_at_end() {
            if self.consume(&Token::Semicolon) {
                continue;
            }
            stmts.push(self.parse_statement()?);
        }

        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Stmt, ParserError> {
        let stmt = match self.current_token() {
            Token::Select | Token::With => Stmt::Select(self.parse_select()?),

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
