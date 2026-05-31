use crate::ast::*;

/// Represents a DML statement to modify existing table rows (`UPDATE`).
///
/// Supports conditional modifications (`WHERE`), joins against supplementary tables (`FROM`),
/// assignment lists, and retrieving modified columns (`RETURNING`).
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStmt {
    /// The target table being modified.
    pub table: TableRef,
    /// The list of column assignments defining new values (e.g. `price = price * 1.1`).
    pub assignment: Vec<Assignment>,
    /// Optional supplementary tables used for join conditions in the update (PostgreSQL `FROM` clause).
    pub from: Vec<TableRef>,
    /// Optional filter condition specifying which rows to update (`WHERE`).
    pub where_: Option<Expr>,
    /// Optional project select items returned after successful modification (`RETURNING`).
    pub returning: Option<SelectItem>,
}
