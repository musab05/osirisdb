use crate::ast::*;

/// Represents a DML statement to delete rows from a table (`DELETE FROM`).
///
/// Supports conditional removals (`WHERE`), joined dependency checks (`USING`),
/// common table expressions (`WITH`), and returning deleted records (`RETURNING`).
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteStmt {
    /// The target table to delete rows from.
    pub table: TableRef,
    /// Optional supplementary tables to join against during evaluation (PostgreSQL `USING` clause).
    pub using: Vec<TableRef>,
    /// Optional filter condition specifying which rows to delete (`WHERE`).
    pub where_: Option<Expr>,
    /// Optional select list items returned after successful deletion (`RETURNING`).
    pub returning: Vec<SelectItem>,
    /// Common Table Expressions defining temporary views for the delete context (`WITH`).
    pub cte: Vec<Cte>,
}
