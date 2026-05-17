use crate::{
    ast::{BinOpKind, DataType, Expr, UnaryOpKind, Value},
    lexer::token::Token,
    parser::{
        binding_power::{infix_binding_power, prefix_binding_power},
        parser::Parser,
        parser_error::ParserError,
    },
};

impl Parser {
    pub fn parse_expr_lists(&mut self) -> Result<Vec<Expr>, ParserError> {
        let mut exprs = vec![];

        loop {
            exprs.push(self.parse_expr()?);
            if !self.consume(&Token::Comma) {
                break;
            }
        }

        Ok(exprs)
    }

    pub fn parse_expr(&mut self) -> Result<Expr, ParserError> {
        self.parser_expr_bp(0)
    }

    fn parser_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ParserError> {
        let mut lhs = self.parse_prefix()?;

        loop {
            if let Some(expr) = self.try_parse_postfix(lhs.clone())? {
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
                            "Expected table name before '.'",
                            self.current.span.clone(),
                        ));
                    }
                };
                lhs = Expr::Column { table, name };
                continue;
            }

            if token == Token::DoubleColon {
                let ty = self.parse_data_type()?;
                lhs = Expr::Cast {
                    expr: Box::new(lhs),
                    ty,
                };
                continue;
            }

            let op = self.token_to_binop(&token)?;
            let rhs = self.parser_expr_bp(r_bp)?;
            lhs = Expr::BinOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
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
            return Ok(Expr::UnaryOp {
                op,
                expr: Box::new(expr),
            });
        }

        match token {
            Token::IntLit(n) => {
                self.advance();
                Ok(Expr::Literal(Value::Int(n)))
            }
            Token::FloatLit(f) => {
                self.advance();
                Ok(Expr::Literal(Value::Float(f)))
            }
            Token::StringLit(s) => {
                self.advance();
                Ok(Expr::Literal(Value::String(s)))
            }
            Token::True => {
                self.advance();
                Ok(Expr::Literal(Value::Boolean(true)))
            }
            Token::False => {
                self.advance();
                Ok(Expr::Literal(Value::Boolean(false)))
            }
            Token::Null => {
                self.advance();
                Ok(Expr::Literal(Value::Null))
            }

            Token::LParen => {
                self.advance();
                let expr = if self.current_token() == &Token::Select {
                    let subq = self.parse_select()?;
                    Expr::Subquery(Box::new(subq))
                } else {
                    self.parse_expr()?
                };

                self.expect(Token::RParen)?;
                Ok(expr)
            }

            Token::Exists => {
                self.advance();
                let negated = false;
                self.expect(Token::LParen)?;
                let subq = self.parse_select()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Exists {
                    subq: Box::new(subq),
                    negated,
                })
            }

            Token::Cast => {
                self.advance();
                self.expect(Token::LParen)?;
                let expr = self.parse_expr()?;
                self.expect(Token::As)?;
                let ty = self.parse_data_type()?;
                self.expect(Token::RParen)?;
                Ok(Expr::Cast {
                    expr: Box::new(expr),
                    ty,
                })
            }

            Token::Case => self.parse_case(),

            Token::Ident(name) | Token::QuotedIdent(name) => {
                self.advance();

                if self.consume(&Token::LParen) {
                    let args = if self.current_token() == &Token::RParen {
                        vec![]
                    } else if self.current_token() == &Token::Star {
                        self.advance();
                        vec![Expr::Wildcard]
                    } else {
                        self.parse_expr_lists()?
                    };

                    self.expect(Token::RParen)?;
                    Ok(Expr::FuncCall { name, args })
                } else {
                    Ok(Expr::Column { table: None, name })
                }
            }

            _ => Err(ParserError::new(
                format!("Unexpected token in expression: {:?}", token),
                self.current.span.clone(),
            )),
        }
    }

    fn try_parse_postfix(&mut self, lhs: Expr) -> Result<Option<Expr>, ParserError> {
        if self.consume(&Token::Is) {
            let negated = self.consume(&Token::Not);
            self.expect(Token::Null)?;
            return Ok(Some(Expr::IsNull {
                expr: Box::new(lhs),
                negated,
            }));
        }

        let negated = if self.current_token() == &Token::Not
            && matches!(self.peek_token(), Token::Between | Token::In | Token::Like)
        {
            self.advance();
            true
        } else {
            false
        };

        if self.consume(&Token::Between) {
            let low = self.parser_expr_bp(4)?;
            self.expect(Token::And)?;
            let high = self.parser_expr_bp(4)?;
            return Ok(Some(Expr::Between {
                expr: Box::new(lhs),
                low: Box::new(low),
                high: Box::new(high),
                negated,
            }));
        }

        if self.consume(&Token::In) {
            self.expect(Token::LParen)?;
            let expr = if self.current_token() == &Token::Select {
                let subq = self.parse_select()?;
                self.expect(Token::RParen)?;
                Expr::InSubquery {
                    expr: Box::new(lhs),
                    subq: Box::new(subq),
                    negated,
                }
            } else {
                let list = self.parse_expr_lists()?;
                self.expect(Token::RParen)?;
                Expr::InList {
                    expr: Box::new(lhs),
                    list,
                    negated,
                }
            };
            return Ok(Some(expr));
        }

        if self.consume(&Token::Like) {
            let pattern = self.parser_expr_bp(4)?;
            let like_expr = Expr::BinOp {
                lhs: Box::new(lhs),
                op: BinOpKind::Like,
                rhs: Box::new(pattern),
            };
            if negated {
                return Ok(Some(Expr::UnaryOp {
                    op: UnaryOpKind::Not,
                    expr: Box::new(like_expr),
                }));
            }
            return Ok(Some(like_expr));
        }

        if negated {
            return Err(ParserError::new(
                "Expected BETWEEN, IN, or LIKE after NOT",
                self.current.span.clone(),
            ));
        }

        Ok(None)
    }

    fn parse_data_type(&mut self) -> Result<DataType, ParserError> {
        match self.current_token().clone() {
            Token::Ident(name) => {
                self.advance();
                match name.to_uppercase().as_str() {
                    "SMALLINT" | "INT2" => Ok(DataType::SmallInt),
                    "INT" | "INTEGER" | "INT4" => Ok(DataType::Int),
                    "BIGINT" | "INT8" => Ok(DataType::BigInt),
                    "BOOLEAN" | "BOOL" => Ok(DataType::Boolean),
                    "FLOAT" | "FLOAT4" | "REAL" => Ok(DataType::Float),
                    "FLOAT8" | "DOUBLE" => Ok(DataType::Double),
                    "TEXT" => Ok(DataType::Text),
                    "JSON" => Ok(DataType::Json),
                    "JSONB" => Ok(DataType::JsonB),
                    "DATE" => Ok(DataType::Date),
                    "TIME" => Ok(DataType::Time),
                    "TIMESTAMP" | "TIMESTAMPTZ" => Ok(DataType::Timestamp),
                    "UUID" => Ok(DataType::UUID),
                    "BINARY" => Ok(DataType::Binary),

                    "CHAR" | "CHARACTER" => {
                        let n = self.parse_optional_length()?;
                        Ok(DataType::Char(n))
                    }
                    "VARCHAR" | "CHARACTER VARYING" => {
                        let n = self.parse_optional_length()?;
                        Ok(DataType::VarChar(n))
                    }
                    "VARBINARY" => {
                        let n = self.parse_optional_length()?;
                        Ok(DataType::VarBinary(n))
                    }
                    "DECIMAL" | "NUMERIC" => {
                        // DECIMAL(precision, scale) — both optional
                        if self.consume(&Token::LParen) {
                            let precision = self.expect_int_literal()? as u8;
                            let scale = if self.consume(&Token::Comma) {
                                Some(self.expect_int_literal()? as u8)
                            } else {
                                None
                            };
                            self.expect(Token::RParen)?;
                            Ok(DataType::Decimal(Some(precision), scale))
                        } else {
                            Ok(DataType::Decimal(None, None))
                        }
                    }
                    // Multi-part custom types: schema.type_name
                    _ => {
                        let mut parts = vec![name];
                        while self.consume(&Token::Dot) {
                            parts.push(self.expect_identifier()?);
                        }
                        Ok(DataType::Custom(parts))
                    }
                }
            }

            // Array: base_type[]
            _ => {
                // If nothing matched, try as custom single ident
                Err(ParserError::new(
                    format!("Expected data type, found {:?}", self.current_token()),
                    self.current.span.clone(),
                ))
            }
        }
    }

    // Helper — parses (n) returning Some(n), or None if no paren
    fn parse_optional_length(&mut self) -> Result<Option<u64>, ParserError> {
        if self.consume(&Token::LParen) {
            let n = self.expect_int_literal()?;
            self.expect(Token::RParen)?;
            Ok(Some(n))
        } else {
            Ok(None)
        }
    }

    // Helper — expects current token to be an integer literal
    fn expect_int_literal(&mut self) -> Result<u64, ParserError> {
        match self.current_token().clone() {
            Token::IntLit(n) if n >= 0 => {
                self.advance();
                Ok(n as u64)
            }
            _ => Err(ParserError::new(
                format!("Expected integer, found {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }

    fn parse_case(&mut self) -> Result<Expr, ParserError> {
        self.advance();

        let operand = if self.current_token() == &Token::When {
            None
        } else {
            Some(Box::new(self.parse_expr()?))
        };

        let mut when_thens = vec![];
        while self.consume(&Token::When) {
            let when = self.parse_expr()?;
            self.expect(Token::Then)?;
            let then = self.parse_expr()?;
            when_thens.push((when, then));
        }

        if when_thens.is_empty() {
            return Err(ParserError::new(
                "CASE requires at least one WHEN clause",
                self.current.span.clone(),
            ));
        }

        let else_ = if self.consume(&Token::Else) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

        self.expect(Token::End)?;

        Ok(Expr::Case {
            operand,
            when_thens,
            else_,
        })
    }

    fn token_to_binop(&self, token: &Token) -> Result<BinOpKind, ParserError> {
        match token {
            Token::Eq => Ok(BinOpKind::Eq),
            Token::Ne => Ok(BinOpKind::Ne),
            Token::Lt => Ok(BinOpKind::Lt),
            Token::Le => Ok(BinOpKind::Le),
            Token::Gt => Ok(BinOpKind::Gt),
            Token::Ge => Ok(BinOpKind::Ge),
            Token::Plus => Ok(BinOpKind::Add),
            Token::Minus => Ok(BinOpKind::Sub),
            Token::Star => Ok(BinOpKind::Mul),
            Token::Slash => Ok(BinOpKind::Div),
            Token::Percent => Ok(BinOpKind::Mod),
            Token::And => Ok(BinOpKind::And),
            Token::Or => Ok(BinOpKind::Or),
            Token::Like => Ok(BinOpKind::Like),
            _ => Err(ParserError::new(
                format!("Token {:?} is not a binary operator", token),
                self.current.span.clone(),
            )),
        }
    }
}
