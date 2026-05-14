use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteStmt {
    pub table: TableRef,
    pub using: Vec<TableRef>,
    pub where_: Option<Expr>,
    pub returning: Vec<SelectItem>,
    pub cte: Vec<Cte>,
}

