use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
/// Represents the SQL `SqlOption` structure in the AST.
pub struct SqlOption {
    pub name: String,
    pub value: Expr,
}
