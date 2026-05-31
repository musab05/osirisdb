use crate::ast::*;

/// The source of rows to insert in an `INSERT` statement.
#[derive(Debug, Clone, PartialEq)]
pub enum InsertSource {
    /// Explicit row literals containing expressions (`VALUES (1, 'alice'), (2, 'bob')`).
    Values(Vec<Vec<Expr>>),
    /// Rows loaded from a nested SELECT query result set (`INSERT INTO ... SELECT ...`).
    Select(Box<SelectStmt>),
    /// Insert a single row filled entirely with default values (`DEFAULT VALUES`).
    DefaultValues,
}
