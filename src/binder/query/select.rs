use crate::{
    ast::{BinOpKind, Expr, ObjectName, SelectItem, SelectStmt, TableRef},
    binder::{
        BindError, Binder,
        bound::{BoundSelectStmt, select::BoundPredicate},
    },
    catalog::objects::ColumnEntry,
    common::symbol::Symbol,
};

impl<'c> Binder<'c> {
    /// Binds a `SELECT` statement.
    ///
    /// Each clause is checked by its own dedicated function below, even
    /// where that function currently does nothing but reject the clause
    /// outright. As support for a clause is added, only that function
    /// needs to change — `bind_select` itself just sequences the checks.
    ///
    /// # Scope
    ///
    /// Currently supported: single-table `FROM`, wildcard projection,
    /// and `WHERE column = literal`. Everything else is rejected.
    ///
    /// # Errors
    ///
    /// Returns [`BindError::UnsupportedSelect`] for any unsupported clause.
    /// Returns [`BindError::TableNotFound`] if the target table does not
    /// exist in the catalog.
    pub fn bind_select(
        &self,
        db: Symbol,
        default_schema: Symbol,
        stmt: SelectStmt,
    ) -> Result<BoundSelectStmt, BindError> {
        Self::check_modifier(&stmt)?;
        Self::check_ctes(&stmt)?;
        Self::check_set_op(&stmt)?;
        Self::check_joins(&stmt)?;
        Self::check_group_by(&stmt)?;
        Self::check_having(&stmt)?;
        Self::check_order_by(&stmt)?;
        Self::check_limit_offset(&stmt)?;

        let table_name = Self::check_from(&stmt)?;
        Self::check_projection(&stmt)?;

        let (schema, table) = table_name.resolve_schema_table(default_schema);

        let table_entry = self
            .catalog
            .get_table(db, schema, table)
            .map_err(|_| BindError::TableNotFound(table))?;

        let columns = table_entry.columns.clone();

        let predicate = Self::bind_where(&stmt, &columns)?;

        Ok(BoundSelectStmt {
            db,
            schema,
            table,
            columns,
            predicate,
        })
    }

    /// `SELECT DISTINCT` / `DISTINCT ON` / `ALL` — not yet supported.
    fn check_modifier(stmt: &SelectStmt) -> Result<(), BindError> {
        if stmt.modifier.is_some() {
            return Err(BindError::UnsupportedSelect);
        }
        Ok(())
    }

    /// `WITH ...` CTEs — not yet supported.
    fn check_ctes(stmt: &SelectStmt) -> Result<(), BindError> {
        if !stmt.ctes.is_empty() {
            return Err(BindError::UnsupportedSelect);
        }
        Ok(())
    }

    /// `UNION` / `INTERSECT` / `EXCEPT` — not yet supported.
    fn check_set_op(stmt: &SelectStmt) -> Result<(), BindError> {
        if stmt.set_op.is_some() {
            return Err(BindError::UnsupportedSelect);
        }
        Ok(())
    }

    /// `JOIN` clauses — not yet supported.
    fn check_joins(stmt: &SelectStmt) -> Result<(), BindError> {
        if !stmt.joins.is_empty() {
            return Err(BindError::UnsupportedSelect);
        }
        Ok(())
    }

    /// `GROUP BY` — not yet supported.
    fn check_group_by(stmt: &SelectStmt) -> Result<(), BindError> {
        if !stmt.group_by.is_empty() {
            return Err(BindError::UnsupportedSelect);
        }
        Ok(())
    }

    /// `HAVING` — not yet supported. Requires `GROUP BY` to exist first,
    /// so this will stay rejected until `check_group_by` allows something
    /// through.
    fn check_having(stmt: &SelectStmt) -> Result<(), BindError> {
        if stmt.having.is_some() {
            return Err(BindError::UnsupportedSelect);
        }
        Ok(())
    }

    /// `ORDER BY` — not yet supported.
    fn check_order_by(stmt: &SelectStmt) -> Result<(), BindError> {
        if !stmt.order_by.is_empty() {
            return Err(BindError::UnsupportedSelect);
        }
        Ok(())
    }

    /// `LIMIT` / `OFFSET` — not yet supported.
    fn check_limit_offset(stmt: &SelectStmt) -> Result<(), BindError> {
        if stmt.limit.is_some() || stmt.offset.is_some() {
            return Err(BindError::UnsupportedSelect);
        }
        Ok(())
    }

    /// `FROM` — exactly one named table, no subqueries, no comma-joins.
    /// Returns the table's `ObjectName` for the caller to resolve against
    /// `db`/`default_schema`.
    fn check_from(stmt: &SelectStmt) -> Result<ObjectName, BindError> {
        if stmt.from.len() != 1 {
            return Err(BindError::UnsupportedSelect);
        }
        match &stmt.from[0] {
            TableRef::Named { name, .. } => Ok(name.clone()),
            _ => Err(BindError::UnsupportedSelect),
        }
    }

    /// Projection list — must be a single bare wildcard (`SELECT *`).
    /// No column lists, aliases, or expressions yet.
    fn check_projection(stmt: &SelectStmt) -> Result<(), BindError> {
        if stmt.columns.len() != 1 {
            return Err(BindError::UnsupportedSelect);
        }
        match &stmt.columns[0] {
            SelectItem::Wildcard => Ok(()),
            _ => Err(BindError::UnsupportedSelect),
        }
    }

    /// `WHERE` — only `column = literal` (either operand order) resolves
    /// to a usable predicate. Anything else is rejected: range comparisons,
    /// `AND`/`OR`, function calls, a column on both sides, etc. This is
    /// deliberately narrow — it exists to enable index point-lookups in
    /// the executor, not general predicate evaluation. There is no
    /// post-filter step yet, so a predicate on a non-indexed column will
    /// still bind successfully here but must be handled explicitly by the
    /// executor (index lookup vs. reject vs. full scan + filter is an
    /// executor-level decision, not a binder-level one).
    fn bind_where(
        stmt: &SelectStmt,
        columns: &[ColumnEntry],
    ) -> Result<Option<BoundPredicate>, BindError> {
        let expr = match &stmt.where_ {
            None => return Ok(None),
            Some(e) => e.clone(),
        };

        let (col_name, value) = match expr {
            Expr::BinOp {
                op: BinOpKind::Eq,
                lhs,
                rhs,
            } => match (*lhs, *rhs) {
                (Expr::Column { name, .. }, Expr::Literal(v)) => (name, v),
                (Expr::Literal(v), Expr::Column { name, .. }) => (name, v),
                _ => return Err(BindError::UnsupportedSelect),
            },
            _ => return Err(BindError::UnsupportedSelect),
        };

        let (column_idx, column_entry) = columns
            .iter()
            .enumerate()
            .find(|(_, c)| c.name == col_name)
            .ok_or(BindError::UnsupportedSelect)?; // TODO: proper ColumnNotFound variant

        Ok(Some(BoundPredicate {
            column_idx,
            column_name: column_entry.name,
            value,
        }))
    }
}
