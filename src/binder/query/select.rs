use crate::{
    ast::{BinOpKind, Expr, SelectItem, SelectStmt, TableRef},
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
    /// # Scope
    ///
    /// Only `SELECT * FROM single_table` is supported. Any modifier,
    /// join, filter, grouping, ordering, pagination, CTE, or set
    /// operation causes an immediate [`BindError::UnsupportedSelect`].
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
        // Reject anything outside SELECT * FROM single_table scope.
        if stmt.modifier.is_some()
            || !stmt.joins.is_empty()
            || !stmt.group_by.is_empty()
            || stmt.having.is_some()
            || !stmt.order_by.is_empty()
            || stmt.limit.is_some()
            || stmt.offset.is_some()
            || !stmt.ctes.is_empty()
            || stmt.set_op.is_some()
        {
            return Err(BindError::UnsupportedSelect);
        }

        // Exactly one FROM table, no subqueries.
        if stmt.from.len() != 1 {
            return Err(BindError::UnsupportedSelect);
        }

        let table_ref = stmt.from.into_iter().next().unwrap();
        let table_name = match table_ref {
            TableRef::Named { name, .. } => name,
            _ => return Err(BindError::UnsupportedSelect),
        };

        // Projection must be a single wildcard — no expressions or aliases.
        if stmt.columns.len() != 1 {
            return Err(BindError::UnsupportedSelect);
        }
        match &stmt.columns[0] {
            SelectItem::Wildcard => {}
            _ => return Err(BindError::UnsupportedSelect),
        }

        // Resolve schema and table, then look up the TableEntry.
        let (schema, table) = table_name.resolve_schema_table(default_schema);

        let table_entry = self
            .catalog
            .get_table(db, schema, table)
            .map_err(|_| BindError::TableNotFound(table))?;

        let columns = table_entry.columns.clone();

        // ── WHERE: only `column = literal` is supported ──
        let predicate = match stmt.where_ {
            None => None,
            Some(expr) => Some(Self::bind_equality_predicate(expr, &columns)?),
        };

        Ok(BoundSelectStmt {
            db,
            schema,
            table,
            columns,
            predicate,
        })
    }

    /// Binds `WHERE col = literal` (or `literal = col`) into a `BoundPredicate`.
    /// Any other shape — range comparisons, AND/OR, function calls, a
    /// column on both sides, etc. — is rejected. This is deliberately
    /// narrow: it exists to enable index point-lookups, not general
    /// predicate evaluation (there's no post-filter step yet).
    fn bind_equality_predicate(
        expr: Expr,
        columns: &[ColumnEntry],
    ) -> Result<BoundPredicate, BindError> {
        let (col_expr, val_expr) = match expr {
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
            .find(|(_, c)| c.name == col_expr)
            .ok_or(BindError::UnsupportedSelect)?; // TODO: proper ColumnNotFound variant
        Ok(BoundPredicate {
            column_idx,
            column_name: column_entry.name,
            value: val_expr,
        })
    }
}
