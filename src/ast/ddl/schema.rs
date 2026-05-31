#[derive(Debug, Clone, PartialEq)]
/// Represents the SQL `CreateSchemaStmt` structure in the AST.
pub struct CreateSchemaStmt {
    pub name: Option<String>,
    pub authorization: Option<String>,
    pub if_not_exists: bool,
}