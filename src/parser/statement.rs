use crate::{
    ast::Statement,
    lexer::{Modifier, token::Token},
    parser::{modifiers::CreateModifiers, parser::Parser, parser_error::ParserError},
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
            Token::Truncate => Statement::TruncateTable(self.parse_truncate()?),
            Token::Create => self.parse_create()?,
            Token::Drop => self.parse_drop()?,
            Token::Alter => self.parse_alter()?,

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

    pub fn parse_create_modifiers(&mut self) -> CreateModifiers {
        let mut m = CreateModifiers {
            or_replace: false,
            temporary: false,
            unlogged: false,
            unique: false,
            materialized: false,
        };

        loop {
            match self.current_token() {
                Token::Or => {
                    self.advance();
                    if let Token::Modifier(Modifier::Replace) = self.current_token() {
                        self.advance();
                        m.or_replace = true;
                    }
                }
                Token::Modifier(Modifier::Replace) => {
                    self.advance();
                    m.or_replace = true;
                }
                Token::Modifier(Modifier::Temporary) | Token::Modifier(Modifier::Temp) => {
                    self.advance();
                    m.temporary = true;
                }
                Token::Modifier(Modifier::Unlogged) => {
                    self.advance();
                    m.unlogged = true;
                }
                Token::Modifier(Modifier::Materialized) => {
                    self.advance();
                    m.materialized = true;
                }
                Token::Modifier(Modifier::Local) | Token::Modifier(Modifier::Global) => {
                    self.advance(); // ignore scope hints
                }
                Token::Unique => {
                    self.advance();
                    m.unique = true;
                }
                _ => break,
            }
        }

        m
    }
}
