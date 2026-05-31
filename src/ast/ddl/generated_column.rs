use crate::ast::*;

/// Represents the definition of an SQL generated/computed column (e.g. `GENERATED ALWAYS AS (age * 2) STORED`).
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedColumn {
    /// The expression used to compute the column value.
    pub expr: Expr,
    /// If `true`, the value is calculated when rows are written and physically stored (`STORED`).
    pub stored: bool,
}
