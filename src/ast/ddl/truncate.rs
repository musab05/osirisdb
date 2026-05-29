use crate::ast::*;

#[derive(Debug, Clone, PartialEq)]
pub struct TruncateStmt {
    pub tables: Vec<ObjectName>,
    pub restart_identity: bool,
    pub behaviour: Option<DropBehavior>,
}
