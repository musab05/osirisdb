use crate::{
    ast::{BinOpKind, DataType, DropBehavior, Expr, SqlOption, UnaryOpKind, Value},
    lexer::token::TokenKind,
    parser::{
        expression::binding_power::{infix_binding_power, prefix_binding_power},
        parser::Parser,
        parser_error::ParserError,
    },
};

impl<'a> Parser<'a> {
    /// Executes parsing or lookup for the `parse_expr_lists` operation.
    /// Parses a comma-separated list of SQL expressions (e.g. `1, a + b, 'hello'`).
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

    /// Executes parsing or lookup for the `parse_expr` operation.
    /// Parses an SQL expression starting at the current token position with a binding power of 0.
    pub fn parse_expr(&mut self) -> Result<Expr, ParserError> {
        self.parser_expr_bp(0)
    }

    /// Core Pratt parsing algorithm loop.
    ///
    /// Evaluates operators in top-down operator precedence using a left and right binding
    /// power to guarantee correct algebraic nesting and associativity (e.g., `1 + 2 * 3` becomes `1 + (2 * 3)`).
    fn parser_expr_bp(&mut self, min_bp: u8) -> Result<Expr, ParserError> {
        let mut lhs = self.parse_prefix()?;

        loop {
            // ── Postfix operators ────────────────────────────────────
            // Peek at the token to decide if postfix parsing applies,
            // then pass lhs by move — no clone needed.
            if self.is_postfix_start() {
                match self.try_parse_postfix(lhs)? {
                    (Some(expr), _) => {
                        lhs = expr;
                        continue;
                    }
                    (None, returned_lhs) => {
                        lhs = returned_lhs;
                    }
                }
            }

            // ── Infix operators ──────────────────────────────────────
            // Borrow the token kind for the binding power lookup —
            // no clone unless we actually need to consume it.
            let Some(&(l_bp, r_bp)) = infix_binding_power(self.current_token()).as_ref() else {
                break;
            };

            if l_bp < min_bp {
                break;
            }

            // We've committed to consuming this operator — now clone
            // the kind (cheap: all infix variants are Copy-sized).
            let token = self.current_token().clone();
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

    /// Parses unary prefix operators (`NOT`, `-`) or basic literals/identifiers (e.g., `TRUE`, `123`, `users.name`).
    fn parse_prefix(&mut self) -> Result<Expr, ParserError> {
        // Check prefix binding power by reference — no clone.
        if let Some(r_bp) = prefix_binding_power(self.current_token()) {
            let op = match self.current_token() {
                TokenKind::Not => UnaryOpKind::Not,
                TokenKind::Minus => UnaryOpKind::Minus,
                _ => unreachable!(),
            };
            self.advance();

            let expr = self.parser_expr_bp(r_bp)?;
            return Ok(Expr::UnaryOp {
                op,
                expr: Box::new(expr),
            });
        }

        match self.current_token().clone() {
            TokenKind::IntLit(n) => {
                self.advance();
                Ok(Expr::Literal(Value::Int(n)))
            }
            TokenKind::FloatLit(f) => {
                self.advance();
                Ok(Expr::Literal(Value::Float(f)))
            }
            TokenKind::StringLit => {
                let s = self.expect_string_literal()?;
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
                let name = self.expect_identifier()?;

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
                let name = self.interner.intern("CURRENT");
                Ok(Expr::Column { table: None, name })
            }

            _ => Err(ParserError::new(
                format!("Unexpected token in expression: {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }

    /// Returns `true` if the current token could start a postfix operator.
    ///
    /// Used to avoid passing `lhs` into `try_parse_postfix` (by move)
    /// when there is clearly no postfix to parse.
    fn is_postfix_start(&self) -> bool {
        matches!(
            self.current_token(),
            TokenKind::Is | TokenKind::Between | TokenKind::In | TokenKind::Like | TokenKind::Not
        )
    }

    /// Attempts to parse trailing postfix operators that follow an expression
    /// (e.g., `IS NULL`, `BETWEEN`, `IN`, `LIKE`).
    ///
    /// Takes `lhs` by **move** to avoid cloning. Returns `(Some(expr), _)` if
    /// a postfix was parsed, or `(None, lhs)` to hand the expression back.
    fn try_parse_postfix(&mut self, lhs: Expr) -> Result<(Option<Expr>, Expr), ParserError> {
        if self.consume(&TokenKind::Is) {
            let negated = self.consume(&TokenKind::Not);
            self.expect(TokenKind::Null)?;
            return Ok((
                Some(Expr::IsNull {
                    expr: Box::new(lhs),
                    negated,
                }),
                Expr::Literal(Value::Null), /* unused */
            ));
        }

        let negated = if self.current_token() == &TokenKind::Not
            && matches!(
                self.peek_token(),
                TokenKind::Between | TokenKind::In | TokenKind::Like
            ) {
            self.advance();
            true
        } else {
            false
        };

        if self.consume(&TokenKind::Between) {
            let low = self.parser_expr_bp(4)?;
            self.expect(TokenKind::And)?;
            let high = self.parser_expr_bp(4)?;
            return Ok((
                Some(Expr::Between {
                    expr: Box::new(lhs),
                    low: Box::new(low),
                    high: Box::new(high),
                    negated,
                }),
                Expr::Literal(Value::Null),
            ));
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
            return Ok((Some(expr), Expr::Literal(Value::Null)));
        }

        if self.consume(&TokenKind::Like) {
            let pattern = self.parser_expr_bp(4)?;
            let like_expr = Expr::BinOp {
                lhs: Box::new(lhs),
                op: BinOpKind::Like,
                rhs: Box::new(pattern),
            };
            if negated {
                return Ok((
                    Some(Expr::UnaryOp {
                        op: UnaryOpKind::Not,
                        expr: Box::new(like_expr),
                    }),
                    Expr::Literal(Value::Null),
                ));
            }
            return Ok((Some(like_expr), Expr::Literal(Value::Null)));
        }

        if negated {
            return Err(ParserError::new(
                "Expected BETWEEN, IN, or LIKE after NOT",
                self.current.span.clone(),
            ));
        }

        Ok((None, lhs))
    }

    /// Parses a SQL data type specification, including array brackets and parameter lengths (e.g., `VARCHAR(255)`, `INT[]`).
    ///
    /// Matches directly on [`TokenKind`] variants — no string allocation
    /// or case conversion needed for built-in types.
    pub fn parse_data_type(&mut self) -> Result<DataType, ParserError> {
        let mut base_type = match self.current_token().clone() {
            // ── Integer types ────────────────────────────────────────────
            TokenKind::Smallint | TokenKind::Int2 => {
                self.advance();
                DataType::SmallInt
            }
            TokenKind::Int | TokenKind::Integer | TokenKind::Int4 => {
                self.advance();
                DataType::Int
            }
            TokenKind::Bigint | TokenKind::Int8 => {
                self.advance();
                DataType::BigInt
            }

            // ── Boolean ──────────────────────────────────────────────────
            TokenKind::Boolean | TokenKind::Bool => {
                self.advance();
                DataType::Boolean
            }

            // ── Floating-point types ─────────────────────────────────────
            TokenKind::Float | TokenKind::Float4 | TokenKind::Real => {
                self.advance();
                DataType::Float
            }
            TokenKind::Double => {
                self.advance();
                self.consume(&TokenKind::Precision);
                DataType::Double
            }
            TokenKind::Float8 => {
                self.advance();
                DataType::Double
            }

            // ── Text / String types ──────────────────────────────────────
            TokenKind::Text => {
                self.advance();
                DataType::Text
            }
            TokenKind::Char => {
                self.advance();
                let n = self.parse_optional_length()?;
                DataType::Char(n)
            }
            TokenKind::Character => {
                self.advance();
                if *self.current_token() == TokenKind::Varying {
                    self.advance();
                    let n = self.parse_optional_length()?;
                    DataType::VarChar(n)
                } else {
                    let n = self.parse_optional_length()?;
                    DataType::Char(n)
                }
            }
            TokenKind::Varchar => {
                self.advance();
                let n = self.parse_optional_length()?;
                DataType::VarChar(n)
            }

            // ── Binary types ─────────────────────────────────────────────
            TokenKind::Binary => {
                self.advance();
                DataType::Binary
            }
            TokenKind::VarBinary => {
                self.advance();
                let n = self.parse_optional_length()?;
                DataType::VarBinary(n)
            }

            // ── Exact numeric types ──────────────────────────────────────
            TokenKind::Decimal | TokenKind::Numeric => {
                self.advance();
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

            // ── JSON types ───────────────────────────────────────────────
            TokenKind::Json => {
                self.advance();
                DataType::Json
            }
            TokenKind::Jsonb => {
                self.advance();
                DataType::JsonB
            }

            // ── Temporal types ───────────────────────────────────────────
            TokenKind::Date => {
                self.advance();
                DataType::Date
            }
            TokenKind::Time => {
                self.advance();
                DataType::Time
            }
            TokenKind::Timestamp | TokenKind::Timestamptz => {
                self.advance();
                DataType::Timestamp
            }
            TokenKind::Interval => {
                self.advance();
                DataType::Interval
            }

            // ── Other types ──────────────────────────────────────────────
            TokenKind::Uuid => {
                self.advance();
                DataType::UUID
            }
            TokenKind::Bytea => {
                self.advance();
                DataType::Bytea
            }

            // ── Custom / user-defined types ──────────────────────────────
            TokenKind::Ident | TokenKind::QuotedIdent => {
                let sym = self.expect_identifier()?;
                let mut parts = vec![sym];
                while self.consume(&TokenKind::Dot) {
                    parts.push(self.expect_identifier()?);
                }
                DataType::Custom(parts)
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
    /// Helper method to parse optional length bounds inside parentheses (e.g., parsing `(255)` after `VARCHAR`).
    fn parse_optional_length(&mut self) -> Result<Option<u64>, ParserError> {
        if self.consume(&TokenKind::LParen) {
            let n = self.expect_int_literal()?;
            self.expect(TokenKind::RParen)?;
            Ok(Some(n))
        } else {
            Ok(None)
        }
    }

    /// Parses a conditional `CASE ... WHEN ... THEN ... ELSE ... END` expression block.
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

    /// Maps a token kind to its corresponding binary operator representation (`BinOpKind`).
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

    /// Executes parsing or lookup for the `parse_drop_behaviour` operation.
    /// Parses optional cascade/restrict flags (`CASCADE` / `RESTRICT`).
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

    /// Executes parsing or lookup for the `parse_options_list` operation.
    /// Parses storage parameters list in parentheses (e.g. `(fillfactor = 70, autovacuum_enabled = true)`).
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
