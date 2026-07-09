use crate::{ast::Value, catalog::objects::ColumnEntry, common::symbol::Symbol};

/// Bound `SELECT` statement — resolved and validated, ready for the
/// executor to scan from storage.
///
/// # Scope
///
/// Currently only supports `SELECT * FROM table_name` — a single table,
/// wildcard projection, no filtering, joins, grouping, or ordering. The
/// binder rejects anything outside this shape.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundSelectStmt {
    /// The database the target table belongs to.
    pub db: Symbol,

    /// The schema the target table belongs to.
    pub schema: Symbol,

    /// The name of the table being scanned.
    pub table: Symbol,

    /// The table's column schema, resolved from the catalog. Needed by
    /// the executor to deserialize raw tuple bytes back into [`crate::ast::Value`]s
    /// during the scan.
    pub columns: Vec<ColumnEntry>,

    /// `Some` only for `WHERE col = literal`. `None` means either no WHERE
    /// clause, or the executor must fall back to a full scan (this binder
    /// doesn't yet support post-filtering a non-indexable predicate — see
    /// `bind_select`, which currently rejects anything more complex).
    pub predicate: Option<BoundPredicate>,
}

/// A WHERE clause the binder was able to resolve into something the
/// executor can act on. Only equality on a single column is supported —
/// anything else is rejected at bind time (see `bind_select`).
#[derive(Debug, Clone, PartialEq)]
pub struct BoundPredicate {
    /// Index of the column in `BoundSelectStmt::columns` (not a Symbol —
    /// avoids a name lookup on every row during the executor's scan path).
    pub column_idx: usize,
    pub column_name: Symbol,
    pub value: Value,
}
