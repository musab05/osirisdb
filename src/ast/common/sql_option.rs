use crate::{ast::*, common::symbol::Symbol};
/// Represents the SQL `SqlOption` structure in the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct SqlOption {
    pub name: Symbol,
    pub value: Expr,
}
