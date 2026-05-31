use crate::ast::*;

/// Represents a query expression starting with `SELECT`.
///
/// This is the primary AST node for data retrieval, supporting projections, filters,
/// joins, aggregations, sort criteria, pagination, CTEs (`WITH`), and set operations.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectStmt {
    /// Optional modifier (e.g., `DISTINCT`, `DISTINCT ON (...)`, or `ALL`).
    pub modifier: Option<SelectModifier>,
    /// The select list of projected items or columns (e.g. `u.id, u.name as username`).
    pub columns: Vec<SelectItem>,
    /// The `FROM` clause tables/subqueries. Multiple items represent comma-joins.
    pub from: Vec<TableRef>,
    /// Any explicit joins applied (e.g. `LEFT JOIN orders ON ...`).
    pub joins: Vec<JoinClause>,
    /// The optional `WHERE` filter condition.
    pub where_: Option<Expr>,
    /// Grouping keys for aggregation (`GROUP BY` columns).
    pub group_by: Vec<Expr>,
    /// Optional filter condition on grouped rows (`HAVING`).
    pub having: Option<Expr>,
    /// Optional ordering criteria (`ORDER BY`).
    pub order_by: Vec<OrderItem>,
    /// Optional limit clause restricting returned row count (`LIMIT`).
    pub limit: Option<Expr>,
    /// Optional offset clause skipping rows (`OFFSET`).
    pub offset: Option<Expr>,
    /// Common Table Expressions defined in the `WITH` clause.
    pub ctes: Vec<Cte>,
    /// Optional trailing set operation combining this query with another (e.g., `UNION [ALL]`).
    pub set_op: Option<Box<SetOperation>>,
}

/// Represents modifiers to the select projection list.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectModifier {
    /// Eliminate duplicate rows (`DISTINCT`).
    Distinct,
    /// PostgreSQL-specific distinct filter keeping only the first row of each set matching the expressions (`DISTINCT ON (col1, ...)`).
    DistinctOn(Vec<Expr>),
    /// Explicitly request all matching rows (`ALL`, which is the default).
    All,
}

/// Represents an item projected in the select list.
#[derive(Debug, Clone, PartialEq)]
pub enum SelectItem {
    /// Wildcard projection (`*`) representing all fields.
    Wildcard,
    /// Qualified wildcard projection (e.g. `users.*`) representing all fields of a specific table.
    QualifiedWildcard(Vec<String>),
    /// A single expression projection, optionally renamed with an alias (e.g., `price * 1.15 AS price_with_tax`).
    Expr {
        /// The column expression.
        expr: Expr,
        /// The column alias identifier.
        alias: Option<String>,
    },
}

/// Represents an SQL set operation combining two queries (e.g., `SELECT ... UNION ALL SELECT ...`).
#[derive(Debug, Clone, PartialEq)]
pub struct SetOperation {
    /// The set operator kind (`UNION`, `INTERSECT`, or `EXCEPT`).
    pub op: SetOp,
    /// If `true`, duplicate rows are preserved (`ALL`).
    pub all: bool,
    /// The right-hand side `SELECT` statement of this set operation.
    pub right: Box<SelectStmt>,
}

/// The set operator kinds.
#[derive(Debug, Clone, PartialEq)]
pub enum SetOp {
    /// Combines rows from both queries, removing duplicates unless `ALL` is specified (`UNION`).
    Union,
    /// Returns only rows that are present in both queries (`INTERSECT`).
    Intersect,
    /// Returns rows from the left query that are not present in the right query (`EXCEPT`).
    Except,
}
