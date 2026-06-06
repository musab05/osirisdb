use crate::{
    ast::{
        CreateTriggerStmt, ObjectName, TransitionKind, TriggerEvent, TriggerFunction,
        TriggerInitially, TriggerLevel, TriggerReferencing, TriggerTiming,
    },
    lexer::TokenKind,
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
    /// Parses a `CREATE TRIGGER` statement.
    ///
    /// Syntax:
    /// ```sql
    /// CREATE [OR REPLACE] [CONSTRAINT] TRIGGER [IF NOT EXISTS] name
    ///     BEFORE | AFTER | INSTEAD OF
    ///     INSERT | UPDATE [OF col, ...] | DELETE | TRUNCATE [OR ...]
    ///     ON table_name
    ///     [FROM ref_table]
    ///     [DEFERRABLE | NOT DEFERRABLE]
    ///     [INITIALLY DEFERRED | INITIALLY IMMEDIATE]
    ///     [REFERENCING OLD TABLE AS x NEW TABLE AS y]
    ///     FOR EACH ROW | FOR EACH STATEMENT
    ///     [PRIORITY n]
    ///     [TAGS ('tag1', ...)]
    ///     [ENABLED | DISABLED]
    ///     [WHEN (condition)]
    ///     EXECUTE FUNCTION | PROCEDURE name(args)
    /// ```
    ///
    /// `or_replace` is passed from `parse_create_modifiers` in `create.rs`.
    /// The `TRIGGER` keyword is consumed by the caller.
    pub fn parse_create_trigger(
        &mut self,
        or_replace: bool,
    ) -> Result<CreateTriggerStmt, ParserError> {
        // CONSTRAINT — marks this as a constraint trigger.
        // Constraint triggers support DEFERRABLE, INITIALLY, and FROM clauses.
        let constraint = self.consume(&TokenKind::Constraint);

        // Trigger name — required
        let name = self.expect_identifier()?;

        // Optional IF NOT EXISTS clause
        let if_not_exists = self.parse_if_not_exist()?;

        // BEFORE | AFTER | INSTEAD OF — required, determines when trigger fires.
        // INSTEAD OF is only valid on views.
        let timing = match self.current_token() {
            TokenKind::Before => {
                self.advance();
                TriggerTiming::Before
            }
            TokenKind::After => {
                self.advance();
                TriggerTiming::After
            }
            // INSTEAD OF — two tokens, consume INSTEAD then require OF
            TokenKind::Instead => {
                self.advance();
                self.expect(TokenKind::Of)?;
                TriggerTiming::InsteadOf
            }
            _ => {
                return Err(ParserError::new(
                    format!(
                        "Expected BEFORE, AFTER or INSTEAD OF, got {:?}",
                        self.current_token()
                    ),
                    self.current.span.clone(),
                ));
            }
        };

        // One or more events separated by OR:
        // INSERT | UPDATE [OF col, ...] | DELETE | TRUNCATE
        let events = self.parse_trigger_events()?;

        // ON table_name — required, the table this trigger is attached to
        self.expect(TokenKind::On)?;
        let table = ObjectName(self.parse_qualified_name()?);

        // ── Constraint trigger options ──
        // These are only meaningful when `constraint = true` but we parse
        // them regardless and let the executor validate the combination.

        let mut from_table = None;
        let mut deferrable = None;
        let mut initially = None;

        loop {
            match self.current_token().clone() {
                // FROM ref_table — the table the foreign key references
                TokenKind::From => {
                    self.advance();
                    from_table = Some(ObjectName(self.parse_qualified_name()?));
                }
                // DEFERRABLE — constraint check can be deferred to end of transaction
                TokenKind::Deferrable => {
                    self.advance();
                    deferrable = Some(true);
                }
                // NOT DEFERRABLE — constraint must be checked immediately
                TokenKind::Not => {
                    self.advance();
                    self.expect(TokenKind::Deferrable)?;
                    deferrable = Some(false);
                }
                // INITIALLY DEFERRED | INITIALLY IMMEDIATE
                TokenKind::Initially => {
                    self.advance();
                    initially = Some(match self.current_token() {
                        TokenKind::Deferred => {
                            self.advance();
                            TriggerInitially::Deferred
                        }
                        TokenKind::Immediate => {
                            self.advance();
                            TriggerInitially::Immediate
                        }
                        _ => {
                            return Err(ParserError::new(
                                format!(
                                    "Expected DEFERRED or IMMEDIATE after INITIALLY, got {:?}",
                                    self.current_token()
                                ),
                                self.current.span.clone(),
                            ));
                        }
                    });
                }
                _ => break,
            }
        }

        // REFERENCING OLD TABLE AS x NEW TABLE AS y
        // Gives names to transition tables for AFTER triggers
        let referencing = if self.consume(&TokenKind::Referencing) {
            self.parse_trigger_referencing()?
        } else {
            vec![]
        };

        // FOR EACH ROW | FOR EACH STATEMENT — required
        self.expect(TokenKind::For)?;
        self.expect(TokenKind::Each)?;
        let level = match self.current_token() {
            TokenKind::Row => {
                self.advance();
                TriggerLevel::Row
            }
            TokenKind::Statement => {
                self.advance();
                TriggerLevel::Statement
            }
            _ => {
                return Err(ParserError::new(
                    format!(
                        "Expected ROW or STATEMENT after FOR EACH, got {:?}",
                        self.current_token()
                    ),
                    self.current.span.clone(),
                ));
            }
        };

        // ── Extra new extensions — optional, any order ──

        let mut priority = None;
        let mut tags = vec![];
        let mut enabled = true; // triggers are enabled by default

        loop {
            match self.current_token().clone() {
                // PRIORITY n — execution order when multiple triggers fire.
                // Lower number fires first. Default is 0.
                TokenKind::Priority => {
                    self.advance();
                    priority = Some(self.expect_int()?);
                }
                // TAGS ('tag1', 'tag2') — metadata labels for categorization.
                // Useful for selective enable/disable and tooling.
                TokenKind::Tags => {
                    self.advance();
                    self.expect(TokenKind::LParen)?;
                    loop {
                        tags.push(self.expect_string_literal()?);
                        if !self.consume(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                }
                // ENABLED — trigger fires normally (default)
                TokenKind::Enabled => {
                    self.advance();
                    enabled = true;
                }
                // DISABLED — trigger is created but will not fire.
                // Avoids a separate ALTER TRIGGER ... DISABLE statement.
                TokenKind::Disabled => {
                    self.advance();
                    enabled = false;
                }
                _ => break,
            }
        }

        // Optional WHEN (condition) — trigger only fires when condition is true.
        // In row-level triggers, OLD and NEW refer to the affected row.
        let condition = if self.consume(&TokenKind::When) {
            self.expect(TokenKind::LParen)?;
            let expr = self.parse_expr()?;
            self.expect(TokenKind::RParen)?;
            Some(expr)
        } else {
            None
        };

        // EXECUTE FUNCTION | PROCEDURE name(args) — required
        let function = self.parse_trigger_function()?;

        Ok(CreateTriggerStmt {
            name,
            if_not_exists,
            or_replace,
            constraint,
            timing,
            events,
            table,
            from_table,
            deferrable,
            initially,
            referencing,
            level,
            condition,
            function,
            priority,
            tags,
            enabled,
        })
    }

    /// Parses one or more trigger events separated by OR.
    ///
    /// Syntax: `INSERT | UPDATE [OF col, ...] | DELETE | TRUNCATE [OR ...]`
    ///
    /// UPDATE can optionally specify which columns trigger the event:
    /// `UPDATE OF price, stock` — only fires if those columns change.
    fn parse_trigger_events(&mut self) -> Result<Vec<TriggerEvent>, ParserError> {
        let mut events = vec![];

        loop {
            let event = match self.current_token().clone() {
                TokenKind::Insert => {
                    self.advance();
                    TriggerEvent::Insert
                }
                TokenKind::Delete => {
                    self.advance();
                    TriggerEvent::Delete
                }
                TokenKind::Truncate => {
                    self.advance();
                    TriggerEvent::Truncate
                }
                // UPDATE [OF col1, col2] — column list is optional
                TokenKind::UpdateKw => {
                    self.advance();
                    let cols = if self.consume(&TokenKind::Of) {
                        let mut cols = vec![];
                        loop {
                            cols.push(self.expect_identifier()?);
                            if !self.consume(&TokenKind::Comma) {
                                break;
                            }
                        }
                        cols
                    } else {
                        vec![]
                    };
                    TriggerEvent::Update(cols)
                }
                _ => {
                    return Err(ParserError::new(
                        format!(
                            "Expected INSERT, UPDATE, DELETE or TRUNCATE, got {:?}",
                            self.current_token()
                        ),
                        self.current.span.clone(),
                    ));
                }
            };

            events.push(event);

            // Events are separated by OR: INSERT OR UPDATE OR DELETE
            if !self.consume(&TokenKind::Or) {
                break;
            }
        }

        Ok(events)
    }

    /// Parses the REFERENCING clause for transition tables.
    ///
    /// Syntax: `OLD TABLE AS alias` | `NEW TABLE AS alias` (repeatable)
    ///
    /// Transition tables give the trigger function access to the full
    /// set of affected rows as a queryable table alias.
    fn parse_trigger_referencing(&mut self) -> Result<Vec<TriggerReferencing>, ParserError> {
        let mut refs = vec![];

        loop {
            let kind = match self.current_token().clone() {
                TokenKind::Old => {
                    self.advance();
                    TransitionKind::OldTable
                }
                TokenKind::New => {
                    self.advance();
                    TransitionKind::NewTable
                }
                _ => break,
            };

            // TABLE keyword — required after OLD or NEW
            self.expect(TokenKind::Table)?;

            // AS is optional but conventional
            self.consume(&TokenKind::As);

            let alias = self.expect_identifier()?;
            refs.push(TriggerReferencing { kind, alias });
        }

        Ok(refs)
    }

    /// Parses the EXECUTE FUNCTION/PROCEDURE clause.
    ///
    /// Syntax: `EXECUTE FUNCTION | PROCEDURE name(arg1, arg2, ...)`
    ///
    /// Both FUNCTION and PROCEDURE are accepted for compatibility with
    /// older PostgreSQL syntax which used PROCEDURE.
    /// Arguments must be string literals per PostgreSQL specification.
    fn parse_trigger_function(&mut self) -> Result<TriggerFunction, ParserError> {
        self.expect(TokenKind::Execute)?;

        // FUNCTION or PROCEDURE — both valid, PROCEDURE is older PG syntax
        match self.current_token() {
            TokenKind::Function | TokenKind::Procedure => self.advance(),
            _ => {
                return Err(ParserError::new(
                    format!(
                        "Expected FUNCTION or PROCEDURE after EXECUTE, got {:?}",
                        self.current_token()
                    ),
                    self.current.span.clone(),
                ));
            }
        }

        // Qualified function name e.g. myschema.my_func
        let name = self.parse_qualified_name()?;

        // Argument list — string literals only
        self.expect(TokenKind::LParen)?;
        let mut args = vec![];
        if !matches!(self.current_token(), TokenKind::RParen) {
            loop {
                args.push(self.expect_string_literal()?);
                if !self.consume(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RParen)?;

        Ok(TriggerFunction { name: ObjectName(name), args })
    }
}
