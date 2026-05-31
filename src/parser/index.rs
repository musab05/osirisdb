use crate::{
    ast::{CreateIndexStmt, IndexItem, NullOrdering, ObjectName, Order},
    lexer::TokenKind,
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
/// Executes parsing or lookup for the `parse_create_index` operation.
    pub fn parse_create_index(&mut self, unique: bool) -> Result<CreateIndexStmt, ParserError> {

        let if_not_exist = self.parse_if_not_exist()?;

        let name = if matches!(self.current_token(), TokenKind::Ident | TokenKind::QuotedIdent) {
            Some(self.expect_identifier()?)
        } else {
            None
        };

        self.expect(TokenKind::On)?;

        let table = ObjectName(self.parse_qualified_name()?);

        let method = if self.consume(&TokenKind::Using) {
            Some(self.expect_identifier()?)
        } else {
            None
        };

        self.expect(TokenKind::LParen)?;
        let mut columns = vec![];
        loop {
            let expr = self.parse_expr()?;

            let order = if self.consume(&TokenKind::Asc) {
                Some(Order::Asc)
            } else if self.consume(&TokenKind::Desc) {
                Some(Order::Desc)
            } else {
                None
            };

            let nulls = if self.consume(&TokenKind::Nulls) {
                if self.consume(&TokenKind::First) {
                    Some(NullOrdering::First)
                } else if self.consume(&TokenKind::Last) {
                    Some(NullOrdering::Last)
                } else {
                    return Err(ParserError::new(
                        "Expected FIRST or LAST after NULLS",
                        self.current.span.clone(),
                    ));
                }
            } else {
                None
            };

            columns.push(IndexItem { expr, order, nulls });

            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;

        let include = if matches!(self.current_token(), TokenKind::Ident)
            && self.source[self.current.span.start..self.current.span.end].to_uppercase()
                == "INCLUDE"
        {
            self.advance();
            self.expect(TokenKind::LParen)?;
            let mut cols = vec![];
            loop {
                cols.push(self.expect_identifier()?);
                if !self.consume(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RParen)?;
            cols
        } else {
            vec![]
        };

        let where_ = if self.consume(&TokenKind::Where) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        Ok(CreateIndexStmt {
            unique,
            if_not_exist,
            name,
            table,
            method,
            columns,
            include,
            where_,
        })
    }
}