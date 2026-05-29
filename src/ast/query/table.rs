use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub enum TableRef {
    Named {
        name: Vec<String>,
        alias: Option<String>,
    },

    Subquery {
        query: Box<SelectStmt>,
        alias: Option<String>,
    },
}

