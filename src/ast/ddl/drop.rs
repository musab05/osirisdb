use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
/// Represents the SQL `DropTableStmt` structure in the AST.
pub struct DropTableStmt {
    pub if_exist: bool,
    pub temporary: bool,
    pub names: Vec<ObjectName>,
    pub behaviour: Option<DropBehavior>,
}

