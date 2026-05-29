use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedColumn {
    pub expr: Expr,
    pub stored: bool,
}
