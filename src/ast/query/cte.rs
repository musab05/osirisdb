use crate::{ast::*, common::symbol::Symbol};

/// Represents a Common Table Expression (CTE) in a `WITH` clause.
///
/// CTEs act as temporary, named result sets that can be referenced like regular tables
/// within the main query block (e.g., `WITH active AS (SELECT ...) SELECT * FROM active`).
#[derive(Debug, Clone, PartialEq)]
pub struct Cte {
    /// The temporary table name alias representing this CTE.
    pub name: Symbol,
    /// The subquery defining the rows inside this CTE.
    pub query: SelectStmt,
}
