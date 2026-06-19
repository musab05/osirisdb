use crate::{
    ast::{Expr, InsertSource, InsertStmt, Value},
    binder::{BindError, Binder, bound::insert::BoundInsertStmt},
    common::symbol::Symbol,
};

impl<'c> Binder<'c> {
    /// Binds an `INSERT INTO` statement.
    ///
    /// # Validation performed
    ///
    /// 1. Rejects `ON CONFLICT` and `RETURNING` clauses — not yet supported.
    /// 2. Confirms the target table exists in the catalog.
    /// 3. Rejects any [`InsertSource`] other than `Values` — `SELECT` and
    ///    `DEFAULT VALUES` sources are not yet supported.
    /// 4. If an explicit column list was given, confirms every named
    ///    column exists on the table.
    /// 5. Confirms each row has exactly as many values as target columns.
    /// 6. Rejects any value expression other than a literal — column
    ///    references, function calls, and subqueries require an
    ///    expression evaluator that does not exist yet.
    /// 7. Confirms no column left unset by an explicit column list is
    ///    `NOT NULL` (such a column would otherwise be silently inserted
    ///    as `NULL`).
    ///
    /// # Resolution performed
    ///
    /// - Resolves the schema and table name, defaulting to
    ///   `default_schema` if the table name in the statement was
    ///   unqualified.
    /// - If no explicit column list was given, targets all table columns
    ///   in table-declared order.
    /// - Reorders every row's values from the order the user listed them
    ///   in into the table's declared column order, filling any
    ///   unspecified (nullable) column with [`Value::Null`].
    ///
    /// # Errors
    ///
    /// Returns [`BindError`] if any validation fails.
    pub fn bind_insert_table(
        &self,
        db: Symbol,
        default_schema: Symbol,
        stmt: InsertStmt,
    ) -> Result<BoundInsertStmt, BindError> {
        // 1. Reject clauses with no executor support yet — fail fast
        //    before doing any other work.
        if stmt.on_conflict.is_some() {
            return Err(BindError::UnsupportedOnConflict);
        }
        if !stmt.returning.is_empty() {
            return Err(BindError::UnsupportedReturning);
        }

        // 2. Resolve target table and look up its schema from the catalog.
        let (schema, table_name) = stmt.table.resolve_schema_table(default_schema);

        let table = self
            .catalog
            .get_table(db, schema, table_name)
            .map_err(|_| BindError::TableNotFound(table_name))?;

        // Clone the column list so the borrow on `self.catalog` doesn't
        // need to stay alive for the rest of this function.
        let table_columns = table.columns.clone();

        // 3. Only InsertSource::Values is bound to executable form.
        let rows = match stmt.source {
            InsertSource::Values(rows) => rows,
            InsertSource::Select(_) | InsertSource::DefaultValues => {
                return Err(BindError::UnsupportedInsertSource);
            }
        };

        // 4. Determine the target column list, in the order the user
        //    intends to supply values for them.
        //
        //    - No explicit column list: target every table column, in
        //      table-declared order.
        //    - Explicit column list: resolve each named column against
        //      the table, preserving the order the user wrote them in
        //      (this may differ from table-declared order).
        let target_columns: Vec<&crate::catalog::objects::ColumnEntry> = if stmt.columns.is_empty() {
            table_columns.iter().collect()
        } else {
            let mut resolved = Vec::with_capacity(stmt.columns.len());
            for col_name in &stmt.columns {
                let entry = table_columns
                    .iter()
                    .find(|c| c.name == *col_name)
                    .ok_or(BindError::ColumnNotFound(*col_name))?;
                resolved.push(entry);
            }
            resolved
        };

        // 5-7. Bind each row: validate width, convert expressions to
        //      values, then remap into table-declared column order.
        let mut bound_rows = Vec::with_capacity(rows.len());
        for row in rows {
            if row.len() != target_columns.len() {
                return Err(BindError::ColumnCountMismatch {
                    expected: target_columns.len(),
                    found: row.len(),
                });
            }

            // Convert each expression to a Value. Only literals are
            // supported — anything else needs an expression evaluator
            // that does not exist yet.
            let mut user_values = Vec::with_capacity(row.len());
            for expr in row {
                match expr {
                    Expr::Literal(v) => user_values.push(v),
                    _ => return Err(BindError::UnsupportedExpression),
                }
            }

            // Start with a full-width row of NULLs in table-declared
            // order, then place each user-supplied value at its
            // table-declared position.
            let mut full_row: Vec<Value> = vec![Value::Null; table_columns.len()];

            for (target_col, value) in target_columns.iter().zip(user_values.into_iter()) {
                let pos = table_columns
                    .iter()
                    .position(|c| c.name == target_col.name)
                    .expect("target_columns is built from table_columns, position must exist");
                full_row[pos] = value;
            }

            // Any column not covered by the (explicit or implicit) target
            // list is still NULL at this point. That is only valid if the
            // column allows NULLs.
            for col in &table_columns {
                let was_targeted = target_columns.iter().any(|c| c.name == col.name);
                if !was_targeted && !col.nullable {
                    return Err(BindError::MissingNotNullColumn(col.name));
                }
            }

            bound_rows.push(full_row);
        }

        Ok(BoundInsertStmt {
            db,
            schema,
            table: table_name,
            rows: bound_rows,
        })
    }
}