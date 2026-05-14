use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub struct IndexItem {
    pub expr: Expr,
    pub order: Option<Order>,
    pub nulls: Option<NullOrdering>,
}

