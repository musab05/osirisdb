use crate::{
    ast::{
        CreateFunctionStmt, FunctionAccess, FunctionBody, FunctionLanguage, FunctionParallel,
        FunctionParam, FunctionReturn, FunctionVolatility, NullBehavior, ObjectName, ParamMode,
        SecurityMode, SqlOption,
    },
    common::symbol::Symbol,
    lexer::TokenKind,
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
    /// Parses a `CREATE FUNCTION` statement.
    ///
    /// Syntax:
    /// ```sql
    /// CREATE [OR REPLACE] FUNCTION [IF NOT EXISTS] name (params)
    ///     RETURNS type | VOID | SETOF type | TABLE (...) | TRIGGER
    ///     LANGUAGE sql | plpgsql | plpython | ...
    ///     [VOLATILE | STABLE | IMMUTABLE]
    ///     [STRICT | CALLED ON NULL INPUT | RETURNS NULL ON NULL INPUT]
    ///     [SECURITY DEFINER | SECURITY INVOKER]
    ///     [PARALLEL SAFE | UNSAFE | RESTRICTED]
    ///     [COST n] [ROWS n]
    ///     [SET param = value]
    ///     [ACCESS PUBLIC | PRIVATE | RESTRICTED]
    ///     [RAISES exception, ...]
    ///     AS $$ body $$ | BEGIN...END
    /// ```
    ///
    /// `or_replace` is passed from `parse_create_modifiers` in `create.rs`.
    /// The `FUNCTION` keyword is consumed by the caller.
    pub fn parse_create_function(
        &mut self,
        or_replace: bool,
    ) -> Result<CreateFunctionStmt, ParserError> {
        // Optional IF NOT EXISTS clause
        let if_not_exists = self.parse_if_not_exist()?;

        // Qualified function name — required
        let name = ObjectName(self.parse_qualified_name()?);

        // Parameter list — required even if empty: `func()`
        self.expect(TokenKind::LParen)?;
        let params = self.parse_function_params()?;
        self.expect(TokenKind::RParen)?;

        // RETURNS clause — required for functions
        let returns = self.parse_function_return()?;

        // LANGUAGE clause — required
        let language = self.parse_function_language()?;

        // ── Optional clauses — all have defaults, order independent ──

        let mut volatility = FunctionVolatility::Volatile; // default per SQL standard
        let mut null_behavior = NullBehavior::CalledOnNull; // default per SQL standard
        let mut security = SecurityMode::Invoker; // default per SQL standard
        let mut parallel = FunctionParallel::Unsafe; // default per SQL standard
        let mut cost = None;
        let mut rows = None;
        let mut set_options: Vec<SqlOption> = vec![];
        let mut access = FunctionAccess::Public; // our default
        let mut raises: Vec<Symbol> = vec![];

        loop {
            match self.current_token().clone() {
                // ── Volatility ──

                // VOLATILE — function may have side effects, result may differ
                // each call. Planner cannot cache or inline. This is the default.
                TokenKind::Volatile => {
                    self.advance();
                    volatility = FunctionVolatility::Volatile;
                }
                // STABLE — no side effects, same result within a single scan.
                // Planner can optimize but not pre-evaluate.
                TokenKind::Stable => {
                    self.advance();
                    volatility = FunctionVolatility::Stable;
                }
                // IMMUTABLE — no side effects, always same result for same args.
                // Planner can pre-evaluate calls with constant arguments.
                TokenKind::Immutable => {
                    self.advance();
                    volatility = FunctionVolatility::Immutable;
                }

                // ── Null behavior ──

                // STRICT — returns NULL immediately if any argument is NULL.
                // Shorthand for RETURNS NULL ON NULL INPUT.
                TokenKind::Strict => {
                    self.advance();
                    null_behavior = NullBehavior::Strict;
                }
                // CALLED ON NULL INPUT — function is called normally even with
                // NULL arguments. The function body must handle NULLs itself.
                TokenKind::Called => {
                    self.advance();
                    self.expect(TokenKind::On)?;
                    self.expect(TokenKind::Null)?;
                    self.expect(TokenKind::Input)?;
                    null_behavior = NullBehavior::CalledOnNull;
                }
                // RETURNS NULL ON NULL INPUT — same as STRICT, more explicit form
                TokenKind::Returns => {
                    self.advance();
                    self.expect(TokenKind::Null)?;
                    self.expect(TokenKind::On)?;
                    self.expect(TokenKind::Null)?;
                    self.expect(TokenKind::Input)?;
                    null_behavior = NullBehavior::Strict;
                }

                // ── Security ──

                // SECURITY DEFINER — runs with privileges of the function owner.
                // SECURITY INVOKER — runs with privileges of the caller (default).
                TokenKind::Security => {
                    self.advance();
                    security = match self.current_token() {
                        TokenKind::Definer => {
                            self.advance();
                            SecurityMode::Definer
                        }
                        TokenKind::Invoker => {
                            self.advance();
                            SecurityMode::Invoker
                        }
                        _ => {
                            return Err(ParserError::new(
                                format!(
                                    "Expected DEFINER or INVOKER after SECURITY, got {:?}",
                                    self.current_token()
                                ),
                                self.current.span.clone(),
                            ));
                        }
                    };
                }

                // ── Parallelism ──

                // PARALLEL SAFE — can run freely in parallel workers.
                // PARALLEL RESTRICTED — can run in parallel but not in workers.
                // PARALLEL UNSAFE — cannot be parallelized (default).
                TokenKind::Parallel => {
                    self.advance();
                    parallel = match self.current_token() {
                        TokenKind::Safe => {
                            self.advance();
                            FunctionParallel::Safe
                        }
                        TokenKind::Unsafe => {
                            self.advance();
                            FunctionParallel::Unsafe
                        }
                        TokenKind::Restricted => {
                            self.advance();
                            FunctionParallel::Restricted
                        }
                        _ => {
                            return Err(ParserError::new(
                                format!(
                                    "Expected SAFE, UNSAFE or RESTRICTED after PARALLEL, got {:?}",
                                    self.current_token()
                                ),
                                self.current.span.clone(),
                            ));
                        }
                    };
                }

                // COST n — estimated execution cost in cpu_operator_cost units.
                // Default is 100. Used by planner for function call costing.
                TokenKind::Cost => {
                    self.advance();
                    cost = Some(self.expect_float()?);
                }

                // ROWS n — estimated number of rows returned for set-returning
                // functions. Only meaningful when RETURNS SETOF or TABLE.
                TokenKind::Rows => {
                    self.advance();
                    rows = Some(self.expect_float()?);
                }

                // SET param = value — sets a GUC parameter for the duration
                // of the function call, restored on exit.
                TokenKind::Set => {
                    self.advance();
                    let param_name = self.expect_identifier()?;
                    self.expect(TokenKind::Eq)?;
                    let value = self.parse_expr()?;
                    set_options.push(SqlOption {
                        name: param_name,
                        value,
                    });
                }

                // ACCESS PUBLIC | PRIVATE | RESTRICTED
                // Our extension — declare visibility at creation time.
                // Avoids a separate GRANT EXECUTE ON FUNCTION statement.
                TokenKind::Access => {
                    self.advance();
                    access = match self.current_token() {
                        TokenKind::Public => {
                            self.advance();
                            FunctionAccess::Public
                        }
                        TokenKind::Private => {
                            self.advance();
                            FunctionAccess::Private
                        }
                        TokenKind::Restricted => {
                            self.advance();
                            FunctionAccess::Restricted
                        }
                        _ => {
                            return Err(ParserError::new(
                                format!(
                                    "Expected PUBLIC, PRIVATE or RESTRICTED after ACCESS, got {:?}",
                                    self.current_token()
                                ),
                                self.current.span.clone(),
                            ));
                        }
                    };
                }

                // RAISES exception1, exception2
                // Our extension — declares which exceptions this function
                // can raise. Useful for static analysis and documentation.
                TokenKind::Raises => {
                    self.advance();
                    loop {
                        raises.push(self.expect_identifier()?);
                        if !self.consume(&TokenKind::Comma) {
                            break;
                        }
                    }
                }

                // No more recognized clauses
                _ => break,
            }
        }

        // Function body — required, must come last
        let body = self.parse_function_body()?;

        Ok(CreateFunctionStmt {
            name,
            or_replace,
            if_not_exists,
            params,
            returns,
            language,
            body,
            volatility,
            null_behavior,
            security,
            parallel,
            cost,
            rows,
            set_options,
            access,
            raises,
        })
    }

    /// Parses the full parameter list of a function.
    ///
    /// Returns an empty vec for `func()` — checked before entering the loop.
    pub fn parse_function_params(&mut self) -> Result<Vec<FunctionParam>, ParserError> {
        let mut params = vec![];

        // Empty parameter list — exit immediately
        if matches!(self.current_token(), TokenKind::RParen) {
            return Ok(params);
        }

        loop {
            params.push(self.parse_function_param()?);
            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }

        Ok(params)
    }

    /// Parses a single function parameter.
    ///
    /// Syntax: `[mode] [name] type [DEFAULT expr]`
    ///
    /// Mode defaults to `IN` if not specified.
    /// Name is optional — positional params are valid in PostgreSQL.
    /// Default is only valid for `IN` and `INOUT` params.
    fn parse_function_param(&mut self) -> Result<FunctionParam, ParserError> {
        // Optional parameter mode — defaults to In
        let mode = match self.current_token() {
            TokenKind::In => {
                self.advance();
                ParamMode::In
            }
            TokenKind::Out => {
                self.advance();
                ParamMode::Out
            }
            TokenKind::Inout => {
                self.advance();
                ParamMode::InOut
            }
            TokenKind::Variadic => {
                self.advance();
                ParamMode::Variadic
            }
            // No mode keyword — defaults to In, don't advance
            _ => ParamMode::In,
        };

        // Optional parameter name — present if current token is an identifier
        // AND the next token is not a type keyword.
        // e.g. `a INTEGER` → name=a, `INTEGER` → no name
        let name = if matches!(
            self.current_token(),
            TokenKind::Ident | TokenKind::QuotedIdent
        ) && !self.is_type_keyword()
        {
            Some(self.expect_identifier()?)
        } else {
            None
        };

        // Data type — required
        let data_type = self.parse_data_type()?;

        // DEFAULT expr or = expr — both forms accepted
        let default = if self.consume(&TokenKind::Default) || self.consume(&TokenKind::Eq) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        Ok(FunctionParam {
            mode,
            name,
            data_type,
            default,
        })
    }

    /// Checks if the current token is a data type keyword.
    ///
    /// Used to distinguish between a parameter name and a type name
    /// when both could be identifiers. For example in `func(a INTEGER)`,
    /// `a` is the name and `INTEGER` is the type. But in `func(INTEGER)`,
    /// there is no name — `INTEGER` is directly the type.
    fn is_type_keyword(&self) -> bool {
        matches!(
            self.current_token(),
            TokenKind::Int
                | TokenKind::Integer
                | TokenKind::Int2
                | TokenKind::Int4
                | TokenKind::Int8
                | TokenKind::Bigint
                | TokenKind::Smallint
                | TokenKind::Boolean
                | TokenKind::Bool
                | TokenKind::Text
                | TokenKind::Varchar
                | TokenKind::Char
                | TokenKind::Character
                | TokenKind::Real
                | TokenKind::Double
                | TokenKind::Float
                | TokenKind::Float4
                | TokenKind::Float8
                | TokenKind::Numeric
                | TokenKind::Decimal
                | TokenKind::Binary
                | TokenKind::VarBinary
                | TokenKind::Date
                | TokenKind::Time
                | TokenKind::Timestamp
                | TokenKind::Timestamptz
                | TokenKind::Interval
                | TokenKind::Json
                | TokenKind::Jsonb
                | TokenKind::Uuid
                | TokenKind::Bytea
                | TokenKind::Setof
        )
    }

    /// Parses the RETURNS clause of a function.
    ///
    /// Syntax:
    /// ```sql
    /// RETURNS INTEGER              -- scalar type
    /// RETURNS VOID                 -- no return value
    /// RETURNS TRIGGER              -- for trigger functions
    /// RETURNS SETOF users          -- set of an existing type
    /// RETURNS TABLE (id INT, ...)  -- inline table definition
    /// ```
    fn parse_function_return(&mut self) -> Result<FunctionReturn, ParserError> {
        self.expect(TokenKind::Returns)?;

        match self.current_token().clone() {
            // VOID — function returns nothing
            TokenKind::Void => {
                self.advance();
                Ok(FunctionReturn::Void)
            }

            // TRIGGER — used for trigger functions, returns a trigger record
            TokenKind::Trigger => {
                self.advance();
                Ok(FunctionReturn::Trigger)
            }

            // SETOF type — returns a set (multiple rows) of the given type
            TokenKind::Setof => {
                self.advance();
                Ok(FunctionReturn::SetOf(self.parse_data_type()?))
            }

            // TABLE (col type, ...) — returns an inline table definition
            TokenKind::Table => {
                self.advance();
                self.expect(TokenKind::LParen)?;
                let mut cols = vec![];
                loop {
                    cols.push(self.parse_function_param()?);
                    if !self.consume(&TokenKind::Comma) {
                        break;
                    }
                }
                self.expect(TokenKind::RParen)?;
                Ok(FunctionReturn::Table(cols))
            }

            // Scalar type — any valid data type
            _ => Ok(FunctionReturn::Type(self.parse_data_type()?)),
        }
    }

    /// Parses the LANGUAGE clause.
    ///
    /// Syntax: `LANGUAGE sql | plpgsql | plpython | plperl | pltcl | custom`
    ///
    /// Language name is case-insensitive and stored in lowercase.
    /// Unknown languages are stored as `FunctionLanguage::Custom(name)`.
    pub fn parse_function_language(&mut self) -> Result<FunctionLanguage, ParserError> {
        self.expect(TokenKind::Language)?;

        let lang = self.source[self.current.span.start..self.current.span.end].to_lowercase();
        let sym = self.interner.intern(&lang);
        self.advance();

        Ok(match lang.as_str() {
            "sql" => FunctionLanguage::Sql,
            "plpgsql" => FunctionLanguage::PlPgSql,
            "plpython" | "plpython3u" | "plpython2u" => FunctionLanguage::PlPython,
            "plperl" | "plperlu" => FunctionLanguage::PlPerl,
            "pltcl" | "pltclu" => FunctionLanguage::PlTcl,
            _ => FunctionLanguage::Custom(sym),
        })
    }

    /// Parses the function body.
    ///
    /// Two forms are supported:
    ///
    /// PostgreSQL dollar-quoted (backward compatible):
    /// ```sql
    /// AS $$ SELECT a + b $$
    /// AS $body$ BEGIN ... END $body$
    /// ```
    ///
    /// Our cleaner BEGIN...END syntax (no dollar quoting needed):
    /// ```sql
    /// BEGIN
    ///     RETURN a + b;
    /// END
    /// ```
    pub fn parse_function_body(&mut self) -> Result<FunctionBody, ParserError> {
        if self.consume(&TokenKind::As) {
            match self.current_token().clone() {
                // Dollar-quoted body: AS $$ ... $$ or AS $tag$ ... $tag$
                TokenKind::DollarStringLit => {
                    let body_str = &self.source[self.current.span.start..self.current.span.end];
                    let body = self.interner.intern(body_str);
                    self.advance();
                    Ok(FunctionBody::DollarQuoted(body))
                }
                // Plain string body: AS 'body'
                TokenKind::StringLit => {
                    let body = self.expect_string_literal()?;
                    Ok(FunctionBody::SqlExpr(body))
                }
                _ => Err(ParserError::new(
                    format!(
                        "Expected string or dollar-quoted body after AS, got {:?}",
                        self.current_token()
                    ),
                    self.current.span.clone(),
                )),
            }
        } else if self.consume(&TokenKind::Begin) {
            // Our BEGIN...END syntax — collect raw source until matching END.
            // Tracks depth to handle nested BEGIN...END blocks correctly.
            let start = self.current.span.start;
            let mut depth = 1;

            loop {
                match self.current_token() {
                    TokenKind::Begin => {
                        depth += 1;
                        self.advance();
                    }
                    TokenKind::End => {
                        depth -= 1;
                        self.advance();
                        if depth == 0 {
                            break;
                        }
                    }
                    TokenKind::Eof => {
                        return Err(ParserError::new(
                            "Unterminated BEGIN...END block in function body",
                            self.current.span.clone(),
                        ));
                    }
                    _ => {
                        self.advance();
                    }
                }
            }

            let end = self.current.span.start;
            let body_str = &self.source[start..end];
            let body = self.interner.intern(body_str);
            Ok(FunctionBody::BeginEnd(body))
        } else {
            Err(ParserError::new(
                format!(
                    "Expected AS or BEGIN for function body, got {:?}",
                    self.current_token()
                ),
                self.current.span.clone(),
            ))
        }
    }
}
