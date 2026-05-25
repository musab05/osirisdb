use crate::{
    ast::{BinOpKind, DataType, DropBehavior, Expr, SqlOption, UnaryOpKind, Value},
    lexer::token::TokenKind,
    parser::{
        binding_power::{infix_binding_power, prefix_binding_power},
        parser::Parser,
        parser_error::ParserError,
    },
};

impl<'a> Parser<'a> {
    pub fn parse_expr_lists(&mut self) -> Result<Vec<Expr>, ParserError> {
        let mut exprs = vec![];

        loop {
            exprs.push(self.parse_expr()?);
            if !self.consume(&TokenKind::Comma) {
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

            if token == TokenKind::Dot {
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

            if token == TokenKind::DoubleColon {
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
                TokenKind::Not => UnaryOpKind::Not,
                TokenKind::Minus => UnaryOpKind::Minus,
                _ => unreachable!(),
            };

            let expr = self.parser_expr_bp(r_bp)?;
            return Ok(Expr::UnaryOp {
                op,
                expr: Box::new(expr),
            });
        }

        match token {
            TokenKind::IntLit(n) => {
                self.advance();
                Ok(Expr::Literal(Value::Int(n)))
            }
            TokenKind::FloatLit(f) => {
                self.advance();
                Ok(Expr::Literal(Value::Float(f)))
            }
            TokenKind::StringLit => {
                let s = self.source[self.current_span().start..self.current_span().end].to_string();
                self.advance();
                Ok(Expr::Literal(Value::String(s)))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::Literal(Value::Boolean(true)))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::Literal(Value::Boolean(false)))
            }
            TokenKind::Null => {
                self.advance();
                Ok(Expr::Literal(Value::Null))
            }

            TokenKind::LParen => {
                self.advance();
                let expr = if self.current_token() == &TokenKind::Select {
                    let subq = self.parse_select()?;
                    Expr::Subquery(Box::new(subq))
                } else {
                    self.parse_expr()?
                };

                self.expect(TokenKind::RParen)?;
                Ok(expr)
            }

            TokenKind::Exists => {
                self.advance();
                let negated = false;
                self.expect(TokenKind::LParen)?;
                let subq = self.parse_select()?;
                self.expect(TokenKind::RParen)?;
                Ok(Expr::Exists {
                    subq: Box::new(subq),
                    negated,
                })
            }

            TokenKind::Cast => {
                self.advance();
                self.expect(TokenKind::LParen)?;
                let expr = self.parse_expr()?;
                self.expect(TokenKind::As)?;
                let ty = self.parse_data_type()?;
                self.expect(TokenKind::RParen)?;
                Ok(Expr::Cast {
                    expr: Box::new(expr),
                    ty,
                })
            }

            TokenKind::Case => self.parse_case(),

            TokenKind::Ident | TokenKind::QuotedIdent => {
                let name = self.source[self.current_span().start..self.current_span().end].to_string();
                self.advance();

                if self.consume(&TokenKind::LParen) {
                    let args = if self.current_token() == &TokenKind::RParen {
                        vec![]
                    } else if self.current_token() == &TokenKind::Star {
                        self.advance();
                        vec![Expr::Wildcard]
                    } else {
                        self.parse_expr_lists()?
                    };

                    self.expect(TokenKind::RParen)?;
                    Ok(Expr::FuncCall { name, args })
                } else {
                    Ok(Expr::Column { table: None, name })
                }
            }

            // Allow contextual keywords to be used as identifiers in expressions
            // (e.g., column named "action", "zone", "time", etc.)
            TokenKind::Current => {
                self.advance();
                // Handle CURRENT_TIMESTAMP and similar
                let name = "CURRENT".to_string();
                Ok(Expr::Column { table: None, name })
            }

            _ => Err(ParserError::new(
                format!("Unexpected token in expression: {:?}", token),
                self.current.span.clone(),
            )),
        }
    }

    fn try_parse_postfix(&mut self, lhs: Expr) -> Result<Option<Expr>, ParserError> {
        if self.consume(&TokenKind::Is) {
            let negated = self.consume(&TokenKind::Not);
            self.expect(TokenKind::Null)?;
            return Ok(Some(Expr::IsNull {
                expr: Box::new(lhs),
                negated,
            }));
        }

        let negated = if self.current_token() == &TokenKind::Not
            && matches!(self.peek_token(), TokenKind::Between | TokenKind::In | TokenKind::Like)
        {
            self.advance();
            true
        } else {
            false
        };

        if self.consume(&TokenKind::Between) {
            let low = self.parser_expr_bp(4)?;
            self.expect(TokenKind::And)?;
            let high = self.parser_expr_bp(4)?;
            return Ok(Some(Expr::Between {
                expr: Box::new(lhs),
                low: Box::new(low),
                high: Box::new(high),
                negated,
            }));
        }

        if self.consume(&TokenKind::In) {
            self.expect(TokenKind::LParen)?;
            let expr = if self.current_token() == &TokenKind::Select {
                let subq = self.parse_select()?;
                self.expect(TokenKind::RParen)?;
                Expr::InSubquery {
                    expr: Box::new(lhs),
                    subq: Box::new(subq),
                    negated,
                }
            } else {
                let list = self.parse_expr_lists()?;
                self.expect(TokenKind::RParen)?;
                Expr::InList {
                    expr: Box::new(lhs),
                    list,
                    negated,
                }
            };
            return Ok(Some(expr));
        }

        if self.consume(&TokenKind::Like) {
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

    pub fn parse_data_type(&mut self) -> Result<DataType, ParserError> {
        let mut base_type = match self.current_token().clone() {
            TokenKind::Ident => {
                let name = self.source[self.current_span().start..self.current_span().end].to_string();
                self.advance();
                match name.to_uppercase().as_str() {
                    "SMALLINT" | "INT2" => DataType::SmallInt,
                    "INT" | "INTEGER" | "INT4" => DataType::Int,
                    "BIGINT" | "INT8" => DataType::BigInt,
                    "BOOLEAN" | "BOOL" => DataType::Boolean,
                    "FLOAT" | "FLOAT4" | "REAL" => DataType::Float,
                    "DOUBLE" => {
                        self.consume(&TokenKind::Precision);
                        DataType::Double
                    }
                    "FLOAT8" => DataType::Double,
                    "TEXT" => DataType::Text,
                    "JSON" => DataType::Json,
                    "JSONB" => DataType::JsonB,
                    "DATE" => DataType::Date,
                    "TIMESTAMP" | "TIMESTAMPTZ" => DataType::Timestamp,
                    "UUID" => DataType::UUID,
                    "BINARY" => DataType::Binary,

                    "CHAR" => {
                        let n = self.parse_optional_length()?;
                        DataType::Char(n)
                    }
                    "CHARACTER" => {
                        if *self.current_token() == TokenKind::Varying {
                            self.advance();
                            let n = self.parse_optional_length()?;
                            DataType::VarChar(n)
                        } else {
                            let n = self.parse_optional_length()?;
                            DataType::Char(n)
                        }
                    }
                    "VARCHAR" => {
                        let n = self.parse_optional_length()?;
                        DataType::VarChar(n)
                    }
                    "VARBINARY" => {
                        let n = self.parse_optional_length()?;
                        DataType::VarBinary(n)
                    }
                    "DECIMAL" | "NUMERIC" => {
                        // DECIMAL(precision, scale) — both optional
                        if self.consume(&TokenKind::LParen) {
                            let precision = self.expect_int_literal()? as u8;
                            let scale = if self.consume(&TokenKind::Comma) {
                                Some(self.expect_int_literal()? as u8)
                            } else {
                                None
                            };
                            self.expect(TokenKind::RParen)?;
                            DataType::Decimal(Some(precision), scale)
                        } else {
                            DataType::Decimal(None, None)
                        }
                    }
                    // Multi-part custom types: schema.type_name
                    _ => {
                        let mut parts = vec![name];
                        while self.consume(&TokenKind::Dot) {
                            parts.push(self.expect_identifier()?);
                        }
                        DataType::Custom(parts)
                    }
                }
            }

            // Handle TIME as a data type
            TokenKind::Time => {
                self.advance();
                DataType::Time
            }

            _ => {
                return Err(ParserError::new(
                    format!("Expected data type, found {:?}", self.current_token()),
                    self.current.span.clone(),
                ));
            }
        };

        // Handle Array types like int[] or text[][]
        while self.consume(&TokenKind::LBracket) {
            self.expect(TokenKind::RBracket)?;
            base_type = DataType::Array(Box::new(base_type));
        }

        Ok(base_type)
    }

    // Helper — parses (n) returning Some(n), or None if no paren
    fn parse_optional_length(&mut self) -> Result<Option<u64>, ParserError> {
        if self.consume(&TokenKind::LParen) {
            let n = self.expect_int_literal()?;
            self.expect(TokenKind::RParen)?;
            Ok(Some(n))
        } else {
            Ok(None)
        }
    }

    // Helper — expects current token to be an integer literal
    pub fn expect_int_literal(&mut self) -> Result<u64, ParserError> {
        match self.current_token().clone() {
            TokenKind::IntLit(n) if n >= 0 => {
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

        let operand = if self.current_token() == &TokenKind::When {
            None
        } else {
            Some(Box::new(self.parse_expr()?))
        };

        let mut when_thens = vec![];
        while self.consume(&TokenKind::When) {
            let when = self.parse_expr()?;
            self.expect(TokenKind::Then)?;
            let then = self.parse_expr()?;
            when_thens.push((when, then));
        }

        if when_thens.is_empty() {
            return Err(ParserError::new(
                "CASE requires at least one WHEN clause",
                self.current.span.clone(),
            ));
        }

        let else_ = if self.consume(&TokenKind::Else) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };

        self.expect(TokenKind::End)?;

        Ok(Expr::Case {
            operand,
            when_thens,
            else_,
        })
    }

    fn token_to_binop(&self, token: &TokenKind) -> Result<BinOpKind, ParserError> {
        match token {
            TokenKind::Eq => Ok(BinOpKind::Eq),
            TokenKind::Ne => Ok(BinOpKind::Ne),
            TokenKind::Lt => Ok(BinOpKind::Lt),
            TokenKind::Le => Ok(BinOpKind::Le),
            TokenKind::Gt => Ok(BinOpKind::Gt),
            TokenKind::Ge => Ok(BinOpKind::Ge),
            TokenKind::Plus => Ok(BinOpKind::Add),
            TokenKind::Minus => Ok(BinOpKind::Sub),
            TokenKind::Star => Ok(BinOpKind::Mul),
            TokenKind::Slash => Ok(BinOpKind::Div),
            TokenKind::Percent => Ok(BinOpKind::Mod),
            TokenKind::And => Ok(BinOpKind::And),
            TokenKind::Or => Ok(BinOpKind::Or),
            TokenKind::Like => Ok(BinOpKind::Like),
            _ => Err(ParserError::new(
                format!("Token {:?} is not a binary operator", token),
                self.current.span.clone(),
            )),
        }
    }

    pub fn parse_qualified_name(&mut self) -> Result<Vec<String>, ParserError> {
        let mut parts = vec![self.expect_identifier()?];

        while self.consume(&TokenKind::Dot) {
            parts.push(self.expect_identifier()?);
        }
        Ok(parts)
    }

    pub fn parse_drop_behaviour(&mut self) -> Option<DropBehavior> {
        match self.current_token() {
            TokenKind::Cascade => {
                self.advance();
                Some(DropBehavior::Cascade)
            }
            TokenKind::Restrict => {
                self.advance();
                Some(DropBehavior::Restrict)
            }
            _ => None,
        }
    }

    pub fn parse_options_list(&mut self) -> Result<Vec<SqlOption>, ParserError> {
        self.expect(TokenKind::LParen)?;
        let mut options = vec![];
        loop {
            let name = self.expect_identifier()?;
            self.expect(TokenKind::Eq)?;
            let value = self.parse_expr()?;
            options.push(SqlOption { name, value });
            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;
        Ok(options)
    }
}
