use crate::{
    ast::{
        Cte, Expr, JoinClause, JoinType, OrderItem, SelectItem, SelectModifier, SelectStmt, SetOp,
        SetOperation, TableRef,
    },
    lexer::token::Token,
    parser::{parser::Parser, parser_error::ParserError},
};

impl Parser {
    pub fn parse_select(&mut self) -> Result<SelectStmt, ParserError> {
        let ctes = if self.consume(&Token::With) {
            self.parse_ctes()?
        } else {
            vec![]
        };

        self.expect(Token::Select)?;

        let modifier = self.parse_select_modifier()?;

        let columns = self.parse_select_column()?;

        let from = if self.consume(&Token::From) {
            self.parse_table_refs()?
        } else {
            vec![]
        };

        let joins = self.parse_joins()?;

        let where_ = if self.consume(&Token::Where) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        let group_by = if self.consume(&Token::Group) {
            self.expect(Token::By)?;
            self.parse_expr_lists()?
        } else {
            vec![]
        };

        let having = if self.consume(&Token::Having) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        let order_by = if self.consume(&Token::Order) {
            self.expect(Token::By)?;
            self.parse_order_items()?
        } else {
            vec![]
        };

        let limit = if self.consume(&Token::Limit) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        let offset = if self.consume(&Token::Offset) {
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

    fn parse_ctes(&mut self) -> Result<Vec<Cte>, ParserError> {
        let mut ctes = vec![];

        loop {
            let name = self.expect_identifier()?;

            self.expect(Token::As)?;

            self.expect(Token::LParen)?;

            let query = self.parse_select()?;

            self.expect(Token::RParen)?;

            ctes.push(Cte { name, query });

            if !self.consume(&Token::Comma) {
                break;
            }
        }
        Ok(ctes)
    }

    fn parse_select_modifier(&mut self) -> Result<Option<SelectModifier>, ParserError> {
        if self.consume(&Token::All) {
            return Ok(Some(SelectModifier::All));
        }

        if self.consume(&Token::Distinct) {
            if self.consume(&Token::On) {
                self.expect(Token::LParen)?;
                let exprs = self.parse_expr_lists()?;
                self.expect(Token::RParen)?;
                return Ok(Some(SelectModifier::DistinctOn(exprs)));
            }
            return Ok(Some(SelectModifier::Distinct));
        }

        Ok(None)
    }

    fn parse_select_column(&mut self) -> Result<Vec<SelectItem>, ParserError> {
        let mut items = vec![];

        loop {
            let item = if self.consume(&Token::Star) {
                SelectItem::Wildcard
            } else {
                let expr = self.parse_expr()?;

                let item = match &expr {
                    Expr::Column {
                        table: Some(t),
                        name,
                    } if name == "*" => SelectItem::QualifiedWildcard(vec![t.clone()]),

                    _ => {
                        let alias = if self.consume(&Token::As) {
                            Some(self.expect_identifier()?)
                        } else if matches!(self.current_token(), Token::Ident(_)) {
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

            if !self.consume(&Token::Comma) || self.is_select_column_end() {
                break;
            }
        }

        Ok(items)
    }

    fn parse_table_refs(&mut self) -> Result<Vec<TableRef>, ParserError> {
        let mut refs = vec![];

        loop {
            let tref = if self.consume(&Token::LParen) {
                let query = self.parse_select()?;
                self.expect(Token::RParen)?;

                let alias = if self.consume(&Token::As) {
                    Some(self.expect_identifier()?)
                } else if matches!(self.current_token(), Token::Ident(_)) {
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
                while self.consume(&Token::Dot) {
                    name.push(self.expect_identifier()?);
                }

                let alias = if self.consume(&Token::As) {
                    Some(self.expect_identifier()?)
                } else if matches!(self.current_token(), Token::Ident(_)) {
                    Some(self.expect_identifier()?)
                } else {
                    None
                };
                TableRef::Named { name, alias }
            };
            refs.push(tref);

            if !self.consume(&Token::Comma) {
                break;
            }
        }

        Ok(refs)
    }

    fn parse_joins(&mut self) -> Result<Vec<JoinClause>, ParserError> {
        let mut joins = vec![];

        loop {
            let join_type = match self.current_token() {
                Token::Join => {
                    self.advance();
                    JoinType::Inner
                }
                Token::Ident(kw) if kw.eq_ignore_ascii_case("INNER") => {
                    self.advance();
                    self.expect(Token::Join)?;
                    JoinType::Inner
                }
                Token::Ident(kw) if kw.eq_ignore_ascii_case("LEFT") => {
                    self.advance();

                    if matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("OUTER"))
                    {
                        self.advance();
                    }
                    self.expect(Token::Join)?;
                    JoinType::Left
                }
                Token::Ident(kw) if kw.eq_ignore_ascii_case("RIGHT") => {
                    self.advance();

                    if matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("OUTER"))
                    {
                        self.advance();
                    }
                    self.expect(Token::Join)?;
                    JoinType::Right
                }
                Token::Ident(kw) if kw.eq_ignore_ascii_case("FULL") => {
                    self.advance();

                    if matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("OUTER"))
                    {
                        self.advance();
                    }
                    self.expect(Token::Join)?;
                    JoinType::Full
                }
                Token::Ident(kw) if kw.eq_ignore_ascii_case("CROSS") => {
                    self.advance();
                    self.expect(Token::Join)?;
                    JoinType::Cross
                }

                _ => break,
            };

            let table = self.parse_single_table_ref()?;

            let condition = if join_type != JoinType::Cross {
                if self.consume(&Token::On) {
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
    fn parse_single_table_ref(&mut self) -> Result<TableRef, ParserError> {
        if self.consume(&Token::LParen) {
            let query = self.parse_select()?;
            self.expect(Token::RParen)?;

            let alias = if self.consume(&Token::As) {
                Some(self.expect_identifier()?)
            } else if matches!(self.current_token(), Token::Ident(_)) {
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

            while self.consume(&Token::Dot) {
                name.push(self.expect_identifier()?);
            }

            let alias = if self.consume(&Token::As) {
                Some(self.expect_identifier()?)
            } else if matches!(self.current_token(), Token::Ident(_)) {
                Some(self.expect_identifier()?)
            } else {
                None
            };
            Ok(TableRef::Named { name, alias })
        }
    }

    fn parse_order_items(&mut self) -> Result<Vec<OrderItem>, ParserError> {
        let mut items = vec![];

        loop {
            let expr = self.parse_expr()?;

            let asc = if self.consume(&Token::Asc) {
                true
            } else if self.consume(&Token::Desc) {
                false
            } else {
                true
            };

            items.push(OrderItem { expr, asc });

            if !self.consume(&Token::Comma) {
                break;
            }
        }

        Ok(items)
    }

    fn parse_set_op(&mut self) -> Result<Option<Box<SetOperation>>, ParserError> {
        let op = match self.current_token() {
            Token::Union => SetOp::Union,
            Token::Intersect => SetOp::Intersect,
            Token::Except => SetOp::Except,
            _ => return Ok(None),
        };

        self.advance();

        let all = self.consume(&Token::All);

        let right = self.parse_select()?;

        Ok(Some(Box::new(SetOperation {
            op,
            all,
            right: Box::new(right),
        })))
    }

    // Check current token matches without consuming
    fn current_is(&self, token: &Token) -> bool {
        self.current_token() == token
    }

    // Consume current token if it's an identifier, return the name
    fn consume_ident(&mut self) -> Option<String> {
        match self.current_token().clone() {
            Token::Ident(name) | Token::QuotedIdent(name) => {
                self.advance();
                Some(name)
            }
            _ => None,
        }
    }

    // Expect a sequence of tokens in order, fail if any doesn't match
    fn expect_keyword_sequence(&mut self, tokens: &[Token]) -> Result<(), ParserError> {
        for token in tokens {
            self.expect(token.clone())?;
        }
        Ok(())
    }

    fn is_select_column_end(&self) -> bool {
        matches!(
            self.current_token(),
            Token::From
                | Token::Where
                | Token::Group
                | Token::Having
                | Token::Order
                | Token::Limit
                | Token::Offset
                | Token::Eof
                | Token::Semicolon
                | Token::Union
                | Token::Intersect
                | Token::Except
        )
    }
}
