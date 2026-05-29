use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub enum DropBehavior {
    Cascade,
    Restrict,
}
