use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
/// Represents the SQL `OrderItem` structure in the AST.
pub struct OrderItem {
    pub expr: Expr,
    pub asc: bool,
}

#[derive(Debug, Clone, PartialEq)]
/// Represents the `Order` SQL AST enum.
pub enum Order {
    Asc,
    Desc,
}
