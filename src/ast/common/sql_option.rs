use crate::ast::*;
/// Represents the SQL `SqlOption` structure in the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct SqlOption {
    pub name: String,
    pub value: Expr,
}
