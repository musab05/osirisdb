use std::collections::HashMap;

use crate::{
    ast::{DataType, Expr, InsertSource, InsertStmt, Value},
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
    ///   unspecified (nullable) column with its DEFAULT or [`Value::Null`].
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
        let target_columns: Vec<&crate::catalog::objects::ColumnEntry> = if stmt.columns.is_empty()
        {
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

        let mut bound_rows = Vec::with_capacity(rows.len());

        // Build a name → table-position map once so the hot loop below
        // gets O(1) column lookup instead of O(n) .position() each time.
        let col_map: HashMap<Symbol, usize> = table_columns
            .iter()
            .enumerate()
            .map(|(i, col)| (col.name, i))
            .collect();

        // Check NOT NULL / PK columns that are not targeted at all.
        // The schema never changes between rows so this is done once,
        // not once per row.
        for col in &table_columns {
            let was_targeted = target_columns.iter().any(|c| c.name == col.name);
            if !was_targeted && (!col.nullable || col.is_primary_key) {
                return Err(BindError::MissingNotNullColumn(col.name));
            }
        }

        // Pre-evaluate literal DEFAULT expressions once per column.
        // Non-literal defaults (nextval, function calls, etc.) need an
        // evaluator that does not exist yet — they fall back to NULL.
        let defaults: Vec<Option<Value>> = table_columns
            .iter()
            .map(|col| match &col.default {
                Some(Expr::Literal(v)) => Some(v.clone()),
                _ => None,
            })
            .collect();

        // row_idx comes from enumerate — used in type-mismatch error messages.
        for (row_idx, row) in rows.into_iter().enumerate() {
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

            // Type-check each value against its target column's declared
            // type before touching full_row — fail early, nothing to undo.
            for (target_col, value) in target_columns.iter().zip(user_values.iter()) {
                check_type_compat(&target_col.data_type, value, target_col.name, row_idx)?;
            }

            // Initialize each column with its DEFAULT, or NULL if none.
            let mut full_row: Vec<Value> = defaults
                .iter()
                .map(|d| d.clone().unwrap_or(Value::Null))
                .collect();

            // Place user-supplied values at their table-declared positions.
            // col_map lookup is O(1) — no .position() scan.
            for (target_col, value) in target_columns.iter().zip(user_values.into_iter()) {
                let pos = col_map[&target_col.name];
                full_row[pos] = value;
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

// ─────────────────────────────────────────────────────────────────────────────
// Type compatibility check
// ─────────────────────────────────────────────────────────────────────────────

/// Checks that `value` is compatible with `data_type`.
///
/// Called at bind time — before any storage is touched — so the user
/// gets a meaningful error at the right layer instead of a cryptic
/// serialization failure deep in tuple.rs.
///
/// NULL always passes; nullability is enforced separately.
/// Integer literals are accepted for Float/Double columns (implicit widening).
fn check_type_compat(
    data_type: &DataType,
    value: &Value,
    col: Symbol,
    row_idx: usize,
) -> Result<(), BindError> {
    match (data_type, value) {
        // NULL always passes — nullability enforced separately.
        (_, Value::Null) => Ok(()),

        // Integer literal into float column — valid implicit widening.
        (DataType::Float | DataType::Double, Value::Int(_)) => Ok(()),

        // ── Integer types ─────────────────────────────────────────────
        // SmallInt needs a range check — Int(99999) must be rejected.
        (DataType::SmallInt, Value::Int(n)) => {
            if *n >= i16::MIN as i64 && *n <= i16::MAX as i64 {
                Ok(())
            } else {
                Err(BindError::TypeMismatch {
                    col,
                    row: row_idx,
                    expected: "SMALLINT (-32768..32767)",
                    got: "integer out of range",
                })
            }
        }
        (DataType::Int, Value::Int(_)) => Ok(()),
        (DataType::BigInt, Value::Int(_)) => Ok(()),

        // ── Boolean ───────────────────────────────────────────────────
        (DataType::Boolean, Value::Boolean(_)) => Ok(()),

        // ── Floating point ────────────────────────────────────────────
        (DataType::Float, Value::Float(_)) => Ok(()),
        (DataType::Double, Value::Float(_)) => Ok(()),

        // ── Variable-length strings ───────────────────────────────────
        (DataType::VarChar(_) | DataType::Char(_) | DataType::Text, Value::String(_)) => Ok(()),

        // ── Everything else is a type mismatch ────────────────────────
        (expected, actual) => Err(BindError::TypeMismatch {
            col,
            row: row_idx,
            expected: type_name(expected),
            got: value_kind(actual),
        }),
    }
}

fn type_name(dt: &DataType) -> &'static str {
    match dt {
        DataType::SmallInt => "SMALLINT",
        DataType::Int => "INT",
        DataType::BigInt => "BIGINT",
        DataType::Float => "FLOAT",
        DataType::Double => "DOUBLE",
        DataType::Boolean => "BOOLEAN",
        DataType::VarChar(_) => "VARCHAR",
        DataType::Char(_) => "CHAR",
        DataType::Text => "TEXT",
        _ => "unsupported type",
    }
}

fn value_kind(v: &Value) -> &'static str {
    match v {
        Value::Int(_) => "integer",
        Value::Float(_) => "float",
        Value::String(_) => "string",
        Value::Boolean(_) => "boolean",
        Value::BitString(_) => "bit string",
        Value::HexString(_) => "hex string",
        Value::Null => "NULL",
    }
}
