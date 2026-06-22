use crate::{catalog::objects::ColumnEntry, common::symbol::Symbol};

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
}
