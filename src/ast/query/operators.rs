/// Represents a binary operator in an SQL expression.
#[derive(Debug, Clone, PartialEq)]
pub enum BinOpKind {
    /// Equality operator (`=`).
    Eq,
    /// Inequality operator (`!=` or `<>`).
    Ne,
    /// Less-than operator (`<`).
    Lt,
    /// Less-than-or-equal operator (`<=`).
    Le,
    /// Greater-than operator (`>`).
    Gt,
    /// Greater-than-or-equal operator (`>=`).
    Ge,

    /// Addition operator (`+`).
    Add,
    /// Subtraction operator (`-`).
    Sub,
    /// Multiplication operator (`*`).
    Mul,
    /// Division operator (`/`).
    Div,
    /// Modulo / remainder operator (`%`).
    Mod,

    /// Logical conjunction operator (`AND`).
    And,
    /// Logical disjunction operator (`OR`).
    Or,

    /// String pattern matching operator (`LIKE`).
    Like,
    /// Membership inclusion operator (`IN`).
    In,
    /// Range inclusion operator (`BETWEEN`).
    Between,
}

/// Represents a unary operator in an SQL expression.
#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOpKind {
    /// Logical negation operator (`NOT`).
    Not,
    /// Unary positive operator (`+`).
    Plus,
    /// Unary negative operator (`-`).
    Minus,
}
