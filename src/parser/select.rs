use crate::{
    ast::{
        Cte, Expr, JoinClause, JoinType, OrderItem, SelectItem, SelectModifier, SelectStmt, SetOp,
        SetOperation, TableRef,
    },
    lexer::TokenKind,
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
/// Executes parsing or lookup for the `parse_select` operation.
    /// Parses a complete `SELECT` query statement, including CTEs, Joins, grouping, ordering, limits, and set operations.
    pub fn parse_select(&mut self) -> Result<SelectStmt, ParserError> {
        let ctes = if self.consume(&TokenKind::With) {
            self.parse_ctes()?
        } else {
            vec![]
        };

        self.expect(TokenKind::Select)?;

        let modifier = self.parse_select_modifier()?;

        let columns = self.parse_select_column()?;

        let from = if self.consume(&TokenKind::From) {
            self.parse_table_refs()?
        } else {
            vec![]
        };

        let joins = self.parse_joins()?;

        let where_ = if self.consume(&TokenKind::Where) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        let group_by = if self.consume(&TokenKind::Group) {
            self.expect(TokenKind::By)?;
            self.parse_expr_lists()?
        } else {
            vec![]
        };

        let having = if self.consume(&TokenKind::Having) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        let order_by = if self.consume(&TokenKind::Order) {
            self.expect(TokenKind::By)?;
            self.parse_order_items()?
        } else {
            vec![]
        };

        let limit = if self.consume(&TokenKind::Limit) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        let offset = if self.consume(&TokenKind::Offset) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        let set_op = self.parse_set_op()?;

        Ok(SelectStmt {
            modifier,
            columns,
            from,
            joins,
            where_,
            group_by,
            having,
            order_by,
            limit,
            offset,
            ctes,
            set_op,
        })
    }

    /// Parses a list of Common Table Expressions inside a `WITH` clause (e.g., `WITH active AS (SELECT ...), admins AS (SELECT ...)`).
    fn parse_ctes(&mut self) -> Result<Vec<Cte>, ParserError> {
        let mut ctes = vec![];

        loop {
            let name = self.expect_identifier()?;

            self.expect(TokenKind::As)?;

            self.expect(TokenKind::LParen)?;

            let query = self.parse_select()?;

            self.expect(TokenKind::RParen)?;

            ctes.push(Cte { name, query });

            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }
        Ok(ctes)
    }

    /// Parses distinct modifiers (`DISTINCT`, `DISTINCT ON (...)`, or `ALL`).
    fn parse_select_modifier(&mut self) -> Result<Option<SelectModifier>, ParserError> {
        if self.consume(&TokenKind::All) {
            return Ok(Some(SelectModifier::All));
        }

        if self.consume(&TokenKind::Distinct) {
            if self.consume(&TokenKind::On) {
                self.expect(TokenKind::LParen)?;
                let exprs = self.parse_expr_lists()?;
                self.expect(TokenKind::RParen)?;
                return Ok(Some(SelectModifier::DistinctOn(exprs)));
            }
            return Ok(Some(SelectModifier::Distinct));
        }

        Ok(None)
    }

    /// Parses target columns in a projection list, supporting wildcards, aliases, and expressions (e.g., `id, price * 1.1 AS cost`).
    fn parse_select_column(&mut self) -> Result<Vec<SelectItem>, ParserError> {
        let mut items = vec![];

        loop {
            let item = if self.consume(&TokenKind::Star) {
                SelectItem::Wildcard
            } else {
                let expr = self.parse_expr()?;

                let item = match &expr {
                    Expr::Column {
                        table: Some(t),
                        name,
                    } if name == "*" => SelectItem::QualifiedWildcard(vec![t.clone()]),

                    _ => {
                        let alias = if self.consume(&TokenKind::As) {
                            Some(self.expect_identifier()?)
                        } else if matches!(self.current_token(), TokenKind::Ident) {
                            Some(self.expect_identifier()?)
                        } else {
                            None
                        };
                        SelectItem::Expr { expr, alias }
                    }
                };
                item
            };

            items.push(item);

            if !self.consume(&TokenKind::Comma) || self.is_select_column_end() {
                break;
            }
        }

        Ok(items)
    }

    /// Parses table source targets in a `FROM` clause, supporting subqueries and aliases.
    fn parse_table_refs(&mut self) -> Result<Vec<TableRef>, ParserError> {
        let mut refs = vec![];

        loop {
            let tref = if self.consume(&TokenKind::LParen) {
                let query = self.parse_select()?;
                self.expect(TokenKind::RParen)?;

                let alias = if self.consume(&TokenKind::As) {
                    Some(self.expect_identifier()?)
                } else if matches!(self.current_token(), TokenKind::Ident) {
                    Some(self.expect_identifier()?)
                } else {
                    None
                };
                TableRef::Subquery {
                    query: Box::new(query),
                    alias,
                }
            } else {
                let mut name = vec![self.expect_identifier()?];
                while self.consume(&TokenKind::Dot) {
                    name.push(self.expect_identifier()?);
                }

                let alias = if self.consume(&TokenKind::As) {
                    Some(self.expect_identifier()?)
                } else if matches!(self.current_token(), TokenKind::Ident) {
                    Some(self.expect_identifier()?)
                } else {
                    None
                };
                TableRef::Named { name, alias }
            };
            refs.push(tref);

            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }

        Ok(refs)
    }

    /// Parses a sequence of explicit JOIN clauses (e.g., `LEFT JOIN orders ON u.id = o.user_id`).
    fn parse_joins(&mut self) -> Result<Vec<JoinClause>, ParserError> {
        let mut joins = vec![];

        loop {
            let join_type = match self.current_token() {
                TokenKind::Join => {
                    self.advance();
                    JoinType::Inner
                }
                TokenKind::Inner => {
                    self.advance();
                    self.expect(TokenKind::Join)?;
                    JoinType::Inner
                }
                TokenKind::Left => {
                    self.advance();
                    self.consume(&TokenKind::Outer);
                    self.expect(TokenKind::Join)?;
                    JoinType::Left
                }
                TokenKind::Right => {
                    self.advance();
                    self.consume(&TokenKind::Outer);
                    self.expect(TokenKind::Join)?;
                    JoinType::Right
                }
                TokenKind::Full => {
                    self.advance();
                    self.consume(&TokenKind::Outer);
                    self.expect(TokenKind::Join)?;
                    JoinType::Full
                }
                TokenKind::Cross => {
                    self.advance();
                    self.expect(TokenKind::Join)?;
                    JoinType::Cross
                }

                _ => break,
            };

            let table = self.parse_single_table_ref()?;

            let condition = if join_type != JoinType::Cross {
                if self.consume(&TokenKind::On) {
                    Some(self.parse_expr()?)
                } else {
                    None
                }
            } else {
                None
            };

            joins.push(JoinClause {
                join_type,
                table,
                condition,
            });
        }

        Ok(joins)
    }

    // helper used by parse_joins — parses one table ref without comma loop
    /// Parses a single table target or subquery block within a join clause context.
    fn parse_single_table_ref(&mut self) -> Result<TableRef, ParserError> {
        if self.consume(&TokenKind::LParen) {
            let query = self.parse_select()?;
            self.expect(TokenKind::RParen)?;

            let alias = if self.consume(&TokenKind::As) {
                Some(self.expect_identifier()?)
            } else if matches!(self.current_token(), TokenKind::Ident) {
                Some(self.expect_identifier()?)
            } else {
                None
            };
            Ok(TableRef::Subquery {
                query: Box::new(query),
                alias,
            })
        } else {
            let mut name = vec![self.expect_identifier()?];

            while self.consume(&TokenKind::Dot) {
                name.push(self.expect_identifier()?);
            }

            let alias = if self.consume(&TokenKind::As) {
                Some(self.expect_identifier()?)
            } else if matches!(self.current_token(), TokenKind::Ident) {
                Some(self.expect_identifier()?)
            } else {
                None
            };
            Ok(TableRef::Named { name, alias })
        }
    }

    /// Parses sorting specifications in an `ORDER BY` clause (e.g., `price DESC, name ASC`).
    fn parse_order_items(&mut self) -> Result<Vec<OrderItem>, ParserError> {
        let mut items = vec![];

        loop {
            let expr = self.parse_expr()?;

            let asc = if self.consume(&TokenKind::Asc) {
                true
            } else if self.consume(&TokenKind::Desc) {
                false
            } else {
                true
            };

            items.push(OrderItem { expr, asc });

            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }

        Ok(items)
    }

    /// Parses trailing set operators combining queries (`UNION`, `INTERSECT`, or `EXCEPT`).
    fn parse_set_op(&mut self) -> Result<Option<Box<SetOperation>>, ParserError> {
        let op = match self.current_token() {
            TokenKind::Union => SetOp::Union,
            TokenKind::Intersect => SetOp::Intersect,
            TokenKind::Except => SetOp::Except,
            _ => return Ok(None),
        };

        self.advance();

        let all = self.consume(&TokenKind::All);

        let right = self.parse_select()?;

        Ok(Some(Box::new(SetOperation {
            op,
            all,
            right: Box::new(right),
        })))
    }



    // Expect a sequence of tokens in order, fail if any doesn't match
/// Executes parsing or lookup for the `expect_keyword_sequence` operation.
    /// Asserts that a sequence of keyword tokens appears next in order, erroring if any matches fail.
    pub fn expect_keyword_sequence(&mut self, tokens: &[TokenKind]) -> Result<(), ParserError> {
        for token in tokens {
            self.expect(token.clone())?;
        }
        Ok(())
    }

    /// Detects if the current token terminates the `SELECT` column projection list (e.g., reaching `FROM` or `WHERE`).
    fn is_select_column_end(&self) -> bool {
        matches!(
            self.current_token(),
            TokenKind::From
                | TokenKind::Where
                | TokenKind::Group
                | TokenKind::Having
                | TokenKind::Order
                | TokenKind::Limit
                | TokenKind::Offset
                | TokenKind::Eof
                | TokenKind::Semicolon
                | TokenKind::Union
                | TokenKind::Intersect
                | TokenKind::Except
        )
    }
}
