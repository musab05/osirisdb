use crate::{ast::Value, common::symbol::Symbol};

/// Bound `INSERT INTO` statement — resolved and validated, ready for the
/// executor to apply to storage.
///
/// Each row in `rows` has already been reordered to match the target
/// table's column order exactly as declared in the catalog (not
/// necessarily the order the user listed columns/values in the original
/// statement) and padded with [`crate::ast::Value::Null`] for any column
/// not explicitly given a value. This means the executor and storage
/// layers can serialize each row directly against
/// [`crate::catalog::objects::TableEntry::columns`] with no further
/// reordering or lookup.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundInsertStmt {
    /// The database the target table belongs to.
    pub db: Symbol,

    /// The schema the target table belongs to.
    pub schema: Symbol,

    /// The name of the target table.
    pub table: Symbol,

    /// The rows to insert. Each inner `Vec<Value>` has exactly
    /// `TableEntry::columns.len()` entries, in table-declared column order.
    pub rows: Vec<Vec<Value>>,
}
