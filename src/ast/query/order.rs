use crate::ast::*;
/// Represents the SQL `OrderItem` structure in the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct OrderItem {
    pub expr: Expr,
    pub asc: bool,
}

/// Represents the `Order` SQL AST enum.
#[derive(Debug, Clone, PartialEq)]
pub enum Order {
    Asc,
    Desc,
}
