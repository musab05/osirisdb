use crate::{
    ast::Statement,
    lexer::{Modifier, TokenKind},
    parser::{modifiers::CreateModifiers, parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
/// Executes parsing or lookup for the `parse` operation.
    pub fn parse(&mut self) -> Result<Vec<Statement>, ParserError> {
        let mut stmts = vec![];

        while !self.is_at_end() {
            if self.consume(&TokenKind::Semicolon) {
                continue;
            }
            stmts.push(self.parse_statement()?);
        }

        Ok(stmts)
    }

    fn parse_statement(&mut self) -> Result<Statement, ParserError> {
        let stmt = match self.current_token() {
            TokenKind::Select | TokenKind::With => Statement::Select(self.parse_select()?),
            TokenKind::Truncate => Statement::TruncateTable(self.parse_truncate()?),
            TokenKind::Create => self.parse_create()?,
            TokenKind::Drop => self.parse_drop()?,
            TokenKind::Alter => self.parse_alter()?,

            _ => {
                return Err(ParserError::new(
                    format!("Unexpected token {:?}", self.current_token()),
                    self.current.span.clone(),
                ));
            }
        };

        self.consume(&TokenKind::Semicolon);

        Ok(stmt)
    }

/// Executes parsing or lookup for the `parse_create_modifiers` operation.
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
                TokenKind::Or => {
                    self.advance();
                    if let TokenKind::Modifier(Modifier::Replace) = self.current_token() {
                        self.advance();
                        m.or_replace = true;
                    }
                }
                TokenKind::Modifier(Modifier::Replace) => {
                    self.advance();
                    m.or_replace = true;
                }
                TokenKind::Modifier(Modifier::Temporary) | TokenKind::Modifier(Modifier::Temp) => {
                    self.advance();
                    m.temporary = true;
                }
                TokenKind::Modifier(Modifier::Unlogged) => {
                    self.advance();
                    m.unlogged = true;
                }
                TokenKind::Modifier(Modifier::Materialized) => {
                    self.advance();
                    m.materialized = true;
                }
                TokenKind::Modifier(Modifier::Local) | TokenKind::Modifier(Modifier::Global) => {
                    self.advance();
                }
                TokenKind::Unique => {
                    self.advance();
                    m.unique = true;
                }
                _ => break,
            }
        }

        m
    }
}
