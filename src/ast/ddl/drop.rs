use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub struct DropTableStmt {
    pub if_exist: bool,
    pub temporary: bool,
    pub names: Vec<ObjectName>,
    pub behaviour: Option<DropBehavior>,
}

