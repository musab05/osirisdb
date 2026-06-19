use crate::{
    ast::{
        Assignment, ConflictAction, ConflictTarget, InsertSource, InsertStmt, ObjectName,
        OnConflict,
    },
    lexer::TokenKind,
    parser::{Parser, ParserError},
};

impl<'a> Parser<'a> {
    /// Parses an `INSERT INTO` statement.
    ///
    /// # Grammar
    ///
    /// ```text
    /// INSERT INTO table_name [(col1, col2, ...)]
    ///   ( VALUES (expr, ...) [, (expr, ...)]*
    ///   | SELECT ...
    ///   | DEFAULT VALUES )
    /// [ON CONFLICT [(col1, ...) | ON CONSTRAINT name]
    ///    DO NOTHING
    ///    | DO UPDATE SET col = expr [, col = expr]* [WHERE expr]]
    /// [RETURNING select_item [, select_item]*]
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`ParserError`] if any required token is missing or a
    /// sub-clause (column list, row tuple, conflict target/action,
    /// returning list) is malformed.
    pub fn parser_insert(&mut self) -> Result<InsertStmt, ParserError> {
        // parse_statement dispatches to this function without consuming
        // INSERT itself, so it is consumed here.
        self.consume(&TokenKind::Insert);
        self.expect(TokenKind::Into)?;

        let table = ObjectName(self.parse_qualified_name()?);

        let columns = self.parse_insert_columns()?;
        let source = self.parse_insert_source()?;
        let on_conflict = self.parse_on_conflict()?;
        let returning = self.parse_returning()?;

        Ok(InsertStmt {
            table,
            columns,
            source,
            on_conflict,
            returning,
        })
    }

    /// Parses an optional explicit column list: `(col1, col2, ...)`.
    ///
    /// An empty result means no column list was given. Callers (the
    /// binder) interpret this as "all table columns, in table-declared
    /// order".
    fn parse_insert_columns(&mut self) -> Result<Vec<crate::common::symbol::Symbol>, ParserError> {
        let mut columns = vec![];

        if self.consume(&TokenKind::LParen) {
            loop {
                columns.push(self.expect_identifier()?);
                if !self.consume(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RParen)?;
        }

        Ok(columns)
    }

    /// Parses the row-source clause: `VALUES (...)`, `SELECT ...`, or
    /// `DEFAULT VALUES`.
    fn parse_insert_source(&mut self) -> Result<InsertSource, ParserError> {
        match self.current_token() {
            TokenKind::Values => {
                self.advance();
                Ok(InsertSource::Values(self.parse_value_rows()?))
            }
            TokenKind::Select | TokenKind::With => {
                Ok(InsertSource::Select(Box::new(self.parse_select()?)))
            }
            TokenKind::Default => {
                self.advance();
                self.expect(TokenKind::Values)?;
                Ok(InsertSource::DefaultValues)
            }
            _ => Err(ParserError::new(
                format!(
                    "Expected VALUES, SELECT, or DEFAULT VALUES after INSERT INTO, found {:?}",
                    self.current_token()
                ),
                self.current_span().clone(),
            )),
        }
    }

    /// Parses one or more parenthesized, comma-separated row tuples:
    /// `(expr, expr, ...) [, (expr, expr, ...)]*`.
    ///
    /// Assumes `VALUES` has already been consumed by the caller.
    fn parse_value_rows(&mut self) -> Result<Vec<Vec<crate::ast::Expr>>, ParserError> {
        let mut rows = Vec::new();

        loop {
            self.expect(TokenKind::LParen)?;

            let mut row = Vec::new();
            loop {
                row.push(self.parse_expr()?);
                if !self.consume(&TokenKind::Comma) {
                    break;
                }
            }

            self.expect(TokenKind::RParen)?;
            rows.push(row);

            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }

        Ok(rows)
    }

    /// Parses an optional `ON CONFLICT [target] action` clause.
    ///
    /// Returns `Ok(None)` if the statement has no `ON CONFLICT` clause.
    fn parse_on_conflict(&mut self) -> Result<Option<OnConflict>, ParserError> {
        if !self.consume(&TokenKind::On) {
            return Ok(None);
        }
        self.expect(TokenKind::Conflict)?;

        let target = self.parse_conflict_target()?;
        let action = self.parse_conflict_action()?;

        Ok(Some(OnConflict { target, action }))
    }

    /// Parses an optional conflict target: either a column list
    /// `(col1, col2, ...)` or `ON CONSTRAINT constraint_name`.
    ///
    /// Returns `Ok(None)` if neither form is present, which is valid SQL —
    /// `ON CONFLICT DO NOTHING` may omit the target and rely on any
    /// applicable unique or exclusion constraint.
    fn parse_conflict_target(&mut self) -> Result<Option<ConflictTarget>, ParserError> {
        if self.consume(&TokenKind::LParen) {
            let mut cols = vec![self.expect_identifier()?];
            while self.consume(&TokenKind::Comma) {
                cols.push(self.expect_identifier()?);
            }
            self.expect(TokenKind::RParen)?;
            return Ok(Some(ConflictTarget::Columns(cols)));
        }

        if self.current_token() == &TokenKind::On {
            self.advance();
            self.expect(TokenKind::Constraint)?;
            let name = self.expect_identifier()?;
            return Ok(Some(ConflictTarget::Constraints(name)));
        }

        Ok(None)
    }

    /// Parses the conflict action: `DO NOTHING` or
    /// `DO UPDATE SET col = expr, ... [WHERE expr]`.
    fn parse_conflict_action(&mut self) -> Result<ConflictAction, ParserError> {
        self.expect(TokenKind::Do)?;

        if self.consume(&TokenKind::Nothing) {
            return Ok(ConflictAction::DoNothing);
        }

        self.expect(TokenKind::Update)?;
        self.expect(TokenKind::Set)?;

        let mut assignments = Vec::new();
        loop {
            let column = self.expect_identifier()?;
            self.expect(TokenKind::Eq)?;
            let value = self.parse_expr()?;
            assignments.push(Assignment { column, value });

            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }

        let where_ = if self.consume(&TokenKind::Where) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        Ok(ConflictAction::DoUpdate {
            assignments,
            where_,
        })
    }

    /// Parses an optional `RETURNING select_item [, select_item]*` clause.
    ///
    /// Returns an empty `Vec` if no `RETURNING` clause is present.
    fn parse_returning(&mut self) -> Result<Vec<crate::ast::SelectItem>, ParserError> {
        if !self.consume(&TokenKind::Returning) {
            return Ok(vec![]);
        }

        let mut items = Vec::new();
        loop {
            let item = if self.consume(&TokenKind::Star) {
                crate::ast::SelectItem::Wildcard
            } else {
                let expr = self.parse_expr()?;
                let alias = if self.consume(&TokenKind::As) {
                    Some(self.expect_identifier()?)
                } else if matches!(self.current_token(), TokenKind::Ident) {
                    Some(self.expect_identifier()?)
                } else {
                    None
                };
                crate::ast::SelectItem::Expr { expr, alias }
            };

            items.push(item);

            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }

        Ok(items)
    }
}
