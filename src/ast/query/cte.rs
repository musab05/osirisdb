use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub struct Cte {
    pub name: String,
    pub query: SelectStmt,
}

