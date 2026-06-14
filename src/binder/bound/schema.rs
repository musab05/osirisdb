use crate::{ast::CreateSchemaStmt, common::symbol::Symbol};

#[derive(Debug, Clone, PartialEq)]
pub struct BoundCreateSchemaStmt {
    pub name: Option<Symbol>,
    pub authorization: Option<Symbol>,
    pub if_not_exists: bool,
}

impl From<BoundCreateSchemaStmt> for CreateSchemaStmt {
    fn from(b: BoundCreateSchemaStmt) -> Self {
        CreateSchemaStmt {
            name: b.name,
            authorization: b.authorization,
            if_not_exists: b.if_not_exists,
        }
    }
}
