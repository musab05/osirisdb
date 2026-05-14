use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub struct CreateIndexStmt {
    pub unique: bool,
    pub if_not_exist: bool,
    pub name: Option<String>,
    pub table: ObjectName,
    pub method: Option<String>,
    pub columns: Vec<IndexItem>,
    pub include: Vec<String>,
    pub where_: Option<Expr>,
}

