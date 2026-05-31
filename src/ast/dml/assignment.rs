use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
/// Represents the SQL `Assignment` structure in the AST.
pub struct Assignment {
    pub column: String,
    pub value: Expr,
}
