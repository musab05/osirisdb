use crate::{
    ast::{Cte, Expr, SelectModifier, SelectStmt, UnaryOpKind, Value},
    lexer::token::Token,
    parser::{binding_power::{infix_binding_power, prefix_binding_power}, parser::Parser, parser_error::ParserError},
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

    fn parse_expr_lists(&mut self) -> Result<Vec<Expr>, ParserError> {
        let mut exprs = vec![];

        loop {
            exprs.push(self.parse_expr()?);
            if !self.consume(&Token::Comma) {
                break;
            }
        }

        Ok(exprs)
    }

    fn parse_expr(&mut self) -> Result<Expr, ParserError> {
        self.parser_expr_bp(0)
    }

    fn parser_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ParserError> {
        let mut lhs = self.parse_prefix()?;

        loop {
            if let Some(expr) = self.try_parse_prefix(lhs.clone())? {
                lhs = expr;
                continue;
            }

            let token = self.current_token().clone();
            let Some((l_bp, r_bp)) = infix_binding_power(&token) else {
                break;
            };

            if l_bp < min_bp {
                break;
            }

            self.advance();

            if token == Token::Dot {
                let name = self.expect_identifier()?;
                let table = match lhs {
                    Expr::Column { name: t, .. } => Some(t),
                    _ => {
                        return Err(ParserError::new(
                            "Expected table name before '.'".into(),
                            self.current.span.clone(),
                        ));
                    }
                };
                lhs = Expr::Column { table, name };
                continue;
            }

            if token == Token::DoubleColon {
                let ty = self.parse_data_type()?;
                lhs Expr::Cast {expr: Box::new(lhs), ty};
                continue;
            }

            let op = token_to_binop(&token)?;
            let rhs = self.parser_expr_bp(r_bp)?;
            lhs = Expr::BinOp { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
        }

        Ok(lhs)
    }

    fn parse_prefix(&mut self) -> Result<Expr, ParserError> {
        let token = self.current_token().clone();

        if let Some(r_bp) = prefix_binding_power(&token) {
            self.advance();
            let op = match token {
                Token::Not => UnaryOpKind::Not,
                Token::Minus => UnaryOpKind::Minus,
                _ => unreachable!(),
            };

            let expr = self.parser_expr_bp(r_bp)?;
            return Ok(Expr::UnaryOp { op, expr: Box::new(expr) });
        }

        match token {
            Token::IntLit(n) => {self.advance(); Ok(Expr::Literal(Value::Int(n)))}
        }
    }

    fn parse_select_column(&mut self) {}
}
