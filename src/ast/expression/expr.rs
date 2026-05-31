use crate::ast::*;

/// Represents an expression node in the SQL Abstract Syntax Tree (AST).
///
/// Expressions can be evaluated to compute scalar values (e.g. within `SELECT` columns,
/// `WHERE` clauses, or constraint definitions). This enum is recursively defined to allow
/// infinitely nested expressions like `a + (b * c)`.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// A constant value literal (e.g., `'hello'`, `42`, `TRUE`, `NULL`).
    Literal(Value),

    /// A column reference, optionally qualified with a table alias (e.g., `id`, `users.name`).
    Column {
        /// Optional table name or table alias prefix.
        table: Option<String>,
        /// The column name.
        name: String,
    },

    /// A binary operator expression (e.g., `a = b`, `price + tax`).
    BinOp {
        /// The binary operator kind.
        op: BinOpKind,
        /// The left-hand side operand.
        lhs: Box<Expr>,
        /// The right-hand side operand.
        rhs: Box<Expr>,
    },

    /// A unary operator expression (e.g., `-price`, `NOT is_active`).
    UnaryOp {
        /// The unary operator kind.
        op: UnaryOpKind,
        /// The operand.
        expr: Box<Expr>,
    },

    /// A function call (e.g., `COUNT(*)`, `COALESCE(val, 0)`).
    FuncCall {
        /// The function name.
        name: String,
        /// The argument list. Can contain wildcard `Expr::Wildcard` for cases like `COUNT(*)`.
        args: Vec<Expr>,
    },

    /// An `IS NULL` or `IS NOT NULL` comparison expression.
    IsNull {
        /// The expression being checked for nullness.
        expr: Box<Expr>,
        /// If `true`, checks `IS NOT NULL`, otherwise checks `IS NULL`.
        negated: bool,
    },

    /// A range evaluation expression (e.g., `age [NOT] BETWEEN 18 AND 30`).
    Between {
        /// The expression being evaluated.
        expr: Box<Expr>,
        /// The lower boundary of the range (inclusive).
        low: Box<Expr>,
        /// The upper boundary of the range (inclusive).
        high: Box<Expr>,
        /// If `true`, represents `NOT BETWEEN`, otherwise `BETWEEN`.
        negated: bool,
    },

    /// A membership comparison against a static list (e.g., `status [NOT] IN ('active', 'pending')`).
    InList {
        /// The expression being evaluated.
        expr: Box<Expr>,
        /// The list of target values.
        list: Vec<Expr>,
        /// If `true`, represents `NOT IN`, otherwise `IN`.
        negated: bool,
    },

    /// A membership comparison against a subquery (e.g., `user_id [NOT] IN (SELECT id FROM admins)`).
    InSubquery {
        /// The expression being evaluated.
        expr: Box<Expr>,
        /// The nested SELECT query.
        subq: Box<SelectStmt>,
        /// If `true`, represents `NOT IN`, otherwise `IN`.
        negated: bool,
    },

    /// An existence check on a subquery (e.g., `[NOT] EXISTS (SELECT 1 FROM orders WHERE user_id = id)`).
    Exists {
        /// The nested SELECT query.
        subq: Box<SelectStmt>,
        /// If `true`, represents `NOT EXISTS`, otherwise `EXISTS`.
        negated: bool,
    },

    /// A conditional CASE expression (e.g., `CASE score WHEN 10 THEN 'perfect' ELSE 'try again' END`).
    Case {
        /// Optional search operand. If `None`, conditions are boolean expressions (searched CASE).
        operand: Option<Box<Expr>>,
        /// A list of `(condition, result)` pairs representing `WHEN condition THEN result` branches.
        when_thens: Vec<(Expr, Expr)>,
        /// Optional default fallback result representing the `ELSE` branch.
        else_: Option<Box<Expr>>,
    },

    /// An explicit type conversion/cast (e.g., `CAST(age AS VARCHAR)` or `age::varchar`).
    Cast {
        /// The expression to convert.
        expr: Box<Expr>,
        /// The target data type.
        ty: DataType,
    },

    /// A standalone scalar subquery evaluating to a single value (e.g., `(SELECT max(price) FROM products)`).
    Subquery(Box<SelectStmt>),

    /// A wildcard symbol (`*`) representing all columns (e.g. used as `COUNT(*)` function arg).
    Wildcard,
}
