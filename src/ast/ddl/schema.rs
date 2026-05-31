/// Represents the SQL `CreateSchemaStmt` structure in the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateSchemaStmt {
    pub name: Option<String>,
    pub authorization: Option<String>,
    pub if_not_exists: bool,
}