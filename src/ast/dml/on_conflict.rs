use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
/// Represents the SQL `OnConflict` structure in the AST.
pub struct OnConflict {
    pub target: Option<ConflictTarget>,
    pub action: ConflictAction,
}

