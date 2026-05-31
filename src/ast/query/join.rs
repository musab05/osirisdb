use crate::ast::*;

/// Represents an explicit join clause in a `SELECT` statement (e.g. `LEFT JOIN orders ON users.id = orders.user_id`).
#[derive(Debug, Clone, PartialEq)]
pub struct JoinClause {
    /// The join type (e.g., `INNER`, `LEFT OUTER`, `CROSS`).
    pub join_type: JoinType,
    /// The right-hand side table reference being joined.
    pub table: TableRef,
    /// The filter condition representing the `ON` clause. Unused for `CROSS JOIN`.
    pub condition: Option<Expr>,
}

/// The type of join to perform.
#[derive(Debug, Clone, PartialEq)]
pub enum JoinType {
    /// Standard inner join matching rows in both tables (`INNER JOIN`).
    Inner,
    /// Left outer join returning all rows from the left table and matched rows from the right table (`LEFT [OUTER] JOIN`).
    Left,
    /// Right outer join returning all rows from the right table and matched rows from the left table (`RIGHT [OUTER] JOIN`).
    Right,
    /// Full outer join returning all rows when there is a match in either left or right table (`FULL [OUTER] JOIN`).
    Full,
    /// Cartesian product of both tables (`CROSS JOIN`).
    Cross,
}
