use crate::{
    ast::{
        CreateProcedureStmt, FunctionAccess,
        ObjectName, SecurityMode, SqlOption,
    },
    common::symbol::Symbol,
    lexer::TokenKind,
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
    /// Parses a `CREATE PROCEDURE` statement.
    ///
    /// Syntax:
    /// ```sql
    /// CREATE [OR REPLACE] PROCEDURE [IF NOT EXISTS] name (params)
    ///     LANGUAGE sql | plpgsql | ...
    ///     [SECURITY DEFINER | SECURITY INVOKER]
    ///     [SET param = value]
    ///     [ACCESS PUBLIC | PRIVATE | RESTRICTED]
    ///     [RAISES exception, ...]
    ///     [TRANSACTION CONTROL]
    ///     [TIMEOUT n]
    ///     [IDEMPOTENT]
    ///     [RETRIES n]
    ///     AS $$ body $$ | BEGIN...END
    /// ```
    ///
    /// Key differences from `CREATE FUNCTION`:
    /// - No `RETURNS` clause — procedures never return values
    /// - No `VOLATILE`/`STABLE`/`IMMUTABLE` — not applicable
    /// - No `STRICT`/`PARALLEL`/`COST`/`ROWS` — not applicable
    /// - Can manage transactions via `COMMIT`/`ROLLBACK` in body
    /// - Called with `CALL proc()` not `SELECT proc()`
    ///
    /// `or_replace` is passed from `parse_create_modifiers` in `create.rs`.
    /// The `PROCEDURE` keyword is consumed by the caller.
    pub fn parse_create_procedure(
        &mut self,
        or_replace: bool,
    ) -> Result<CreateProcedureStmt, ParserError> {
        // Optional IF NOT EXISTS clause
        let if_not_exists = self.parse_if_not_exist()?;

        // Qualified procedure name — required
        let name = ObjectName(self.parse_qualified_name()?);

        // Parameter list — required even if empty: `proc()`
        self.expect(TokenKind::LParen)?;
        let params = self.parse_function_params()?;
        self.expect(TokenKind::RParen)?;

        // LANGUAGE clause — required
        let language = self.parse_function_language()?;

        // ── Optional clauses — all have defaults, order independent ──

        // SECURITY INVOKER is the default per SQL standard
        let mut security = SecurityMode::Invoker;

        // Runtime configuration options set for duration of procedure call
        let mut set_options: Vec<SqlOption> = vec![];

        // PUBLIC access by default — our extension
        let mut access = FunctionAccess::Public;

        // No declared exceptions by default — our extension
        let mut raises: Vec<Symbol> = vec![];

        // false by default — procedure does not manage transactions
        let mut transaction_control = false;

        // No timeout by default — our extension
        let mut timeout: Option<u64> = None;

        // false by default — procedure is not marked idempotent
        let mut idempotent = false;

        // No auto-retry by default — our extension
        let mut retries: Option<u32> = None;

        loop {
            match self.current_token().clone() {
                // SECURITY DEFINER | SECURITY INVOKER
                // DEFINER — runs with privileges of the procedure owner.
                // INVOKER — runs with privileges of the caller (default).
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
                            ))
                        }
                    };
                }

                // SET param = value — sets a GUC parameter for the duration
                // of the procedure call, restored on exit.
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
                // Avoids a separate GRANT EXECUTE ON PROCEDURE statement.
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
                            ))
                        }
                    };
                }

                // RAISES exception1, exception2
                // Our extension — declares which exceptions this procedure
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

                // TRANSACTION CONTROL — two tokens: TRANSACTION then CONTROL.
                // Our extension — explicitly declares that this procedure
                // manages its own transactions via COMMIT/ROLLBACK in the body.
                // PostgreSQL has no way to declare this upfront.
                // Useful for static analysis, tooling, and documentation.
                TokenKind::Transaction => {
                    self.advance();
                    self.expect(TokenKind::Control)?;
                    transaction_control = true;
                }

                // TIMEOUT n — maximum execution time in milliseconds.
                // Our extension — procedure is killed if it exceeds this limit.
                // PostgreSQL requires setting statement_timeout GUC separately.
                TokenKind::Timeout => {
                    self.advance();
                    timeout = Some(self.expect_int_literal()?);
                }

                // IDEMPOTENT — marks the procedure as safe to call multiple
                // times with the same arguments without unintended side effects.
                // Our extension — useful for retry logic and documentation.
                TokenKind::Idempotent => {
                    self.advance();
                    idempotent = true;
                }

                // RETRIES n — number of times to automatically retry this
                // procedure on serialization failure or deadlock.
                // Our extension — PostgreSQL requires application-level retries.
                TokenKind::Retries => {
                    self.advance();
                    retries = Some(self.expect_int_literal()? as u32);
                }

                // No more recognized clauses — stop parsing options
                _ => break,
            }
        }

        // Procedure body — required, must come last.
        // Reuses parse_function_body from function.rs — supports both
        // PostgreSQL AS $$ ... $$ and our cleaner BEGIN...END syntax.
        let body = self.parse_function_body()?;

        Ok(CreateProcedureStmt {
            name,
            or_replace,
            if_not_exists,
            params,
            language,
            security,
            set_options,
            access,
            raises,
            transaction_control,
            timeout,
            idempotent,
            retries,
            body,
        })
    }
}