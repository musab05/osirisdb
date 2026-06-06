use crate::ast::*;
use crate::common::symbol::Symbol;

/// Represents the SQL `Assignment` structure in the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub column: Symbol,
    pub value: Expr,
}
