use crate::ast::*;
/// Represents the SQL `CreateExtensionStmt` structure in the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateExtensionStmt {
    pub name: String,
    pub if_not_exists: bool,
    pub schema: Option<String>,
    pub version: Option<String>,
    pub cascade: bool,
}