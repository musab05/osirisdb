use std::collections;

use crate::{
    ast::{ColumnConstraint, ColumnDef, CreateStmt, GeneratedColumn, TableConstraint, data_types},
    lexer::token::Token,
    parser::{parser::Parser, parser_error::ParserError},
};

impl Parser {
    pub fn parse_create(&mut self) -> Result<CreateStmt, ParserError> {
        self.consume(&Token::Create)?;

        let mut temporary = false;
        let mut unlogged = false;

        match self.current_token() {
            Token::Ident(k)
                if k.eq_ignore_ascii_case("temporary") || k.eq_ignore_ascii_case("temp") =>
            {
                temporary = true;
                self.advance();
            }
            Token::Ident(k) if k.eq_ignore_ascii_case("unlogged") => {
                unlogged = true;
                self.advance();
            }

            _ => {}
        }

        self.expect(Token::Table)?;

        let if_not_exist = self.parse_if_not_exist()?;

        let name = self.parse_qualified_name()?;

        self.expect(Token::LParen)?;

        let mut columns = vec![];
        let mut constraints = vec![];

        loop {
            if *self.current_token() == Token::RParen {
                break;
            }

            if self.is_table_constraint() {
                constraints.push(self.parse_table_constraint()?);
            } else {
                columns.push(self.parse_column_def()?);
            }

            if !self.consume(&Token::Comma) {
                break;
            }

            self.expect(Token::RParen)?;

            Ok(CreateStmt {
                if_not_exist,
                temporary,
                unlogged,
                name,
                columns,
                constraints,
                inherits: vec![],
                partitions: vec![],
                with_options: vec![],
                table_space: None,
                on_commit: None,
                as_query: None,
            })
        }
    }

    fn parse_if_not_exist(&mut self) -> Result<bool, ParserError> {
        if *self.current_token() == Token::Ident("IF".into()) {
            self.advance();
            self.expect(Token::Not)?;
            self.expect(Token::Exists)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn parse_qualified_name(&mut self) -> Result<Vec<String>, ParserError> {
        let mut parts = vec![self.expect_identifier()?];

        while self.consume(&Token::Dot) {
            parts.push(self.expect_identifier()?);
        }
        Ok(parts)
    }

    fn is_table_constraint(&self) -> bool {
        matches!(
            self.current_token(),
            Token::Primary | Token::Foreign | Token::Unique | Token::Check | Token::Ident(_)
        ) && matches!(
            self.current_token(),
            Token::Primary | Token::Foreign | Token::Unique | Token::Check
        ) || matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("constraint"))
    }

    fn parse_column_def(&mut self) -> Result<ColumnDef, ParserError> {
        let name = self.expect_identifier()?;
        let data_type = self.parse_data_type()?;

        let collation = if matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("collate"))
        {
            self.advance();
            Some(self.expect_identifier()?)
        } else {
            None
        };

        let mut constraints = vec![];
        let mut generated = None;

        loop {
            match self.current_token().clone() {
                Token::Ident(ref k) if k.eq_ignore_ascii_case("constraint") => {
                    self.advance();
                    let _constraint_name = self.expect_identifier()?;
                    constraints.push(self.parse_column_constraint()?);
                }
                Token::Ident(ref k) if k.eq_ignore_ascii_case("generated") => {
                    generated = Some(self.parse_generated_column()?);
                }
                Token::Ident(ref k)
                    if k.eq_ignore_ascii_case("autoincrement")
                        || k.eq_ignore_ascii_case("auto_increment") =>
                {
                    self.advance();
                    constraints.push(ColumnConstraint::AutoIncrement);
                }

                Token::Not
                | Token::Null
                | Token::Default
                | Token::Unique
                | Token::Primary
                | Token::References
                | Token::Check => {
                    constraints.push(self.parse_column_constraint()?);
                }
                _ => break,
            }
        }

        Ok(ColumnDef {
            name,
            data_type,
            collation,
            constraints,
            generated,
        })
    }

    fn parse_column_constraint(&mut self) -> Result<ColumnConstraint, ParserError> {
        match self.current_token().clone() {
            Token::Not => {
                self.advance();
                self.expect(Token::Null)?;
                Ok(ColumnConstraint::NotNull)
            }
            Token::Null => {
                self.advance();
                Ok(ColumnConstraint::Null)
            }
            Token::Default => {
                self.advance();
                Ok(ColumnConstraint::Default(self.parse_expr()?))
            }
            Token::Unique => {
                self.advance();
                Ok(ColumnConstraint::Unique)
            }
            Token::Primary => {
                self.advance();
                self.expect(Token::Key)?;
                Ok(ColumnConstraint::PrimaryKey)
            }
            Token::Check => {
                self.advance();
                self.expect(Token::LParen)?;
                let expr = self.parse_expr()?;
                self.expect(Token::RParen)?;

                Ok(ColumnConstraint::Check(expr))
            }
            Token::References => {
                self.advance();
                let table = self.parse_qualified_name()?;
                let columns = if *self.current_token() == Token::LParen {
                    self.parse_column_list()?
                } else {
                    vec![]
                };

                let (on_delete, on_update) = self.parse_referential_actions()?;
                Ok(ColumnConstraint::References {
                    table,
                    columns,
                    on_delete,
                    on_update,
                })
            }

            _ => Err(ParserError::new(
                format!("Unknown column constraint: {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }

    fn parse_generated_column(&mut self) -> Result<GeneratedColumn, ParserError> {
        self.advance();

        if !matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("always")) {
            return Err(ParserError::new(
                "Expected ALWAYS after GENERATED",
                self.current.span.clone(),
            ));
        }

        self.advance();

        self.expect(Token::As)?;
        self.expect(Token::LParen)?;
        let expr = self.parse_expr()?;
        self.expect(Token::RParen)?;

        if matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("stored")) {
            self.advance();
        }

        Ok(GeneratedColumn { expr, stored: true })
    }

    fn parse_table_constraint(&mut self) -> Result<TableConstraint, ParserError> {
        
    }
}
