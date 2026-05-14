use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStmt {
    pub table: TableRef,
    pub assignment: Vec<Assignment>,
    pub from: Vec<TableRef>,
    pub where_: Option<Expr>,
    pub returning: Option<SelectItem>,
}

