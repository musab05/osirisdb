use crate::common::symbol::Symbol;

/// Represents the SQL `CreateSchemaStmt` structure in the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateSchemaStmt {
    pub name: Option<Symbol>,
    pub authorization: Option<Symbol>,
    pub if_not_exists: bool,
}