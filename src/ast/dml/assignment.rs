use crate::ast::*;
/// Represents the SQL `Assignment` structure in the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub column: String,
    pub value: Expr,
}
