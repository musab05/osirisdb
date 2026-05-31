use crate::ast::*;
/// Represents the SQL `OnConflict` structure in the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct OnConflict {
    pub target: Option<ConflictTarget>,
    pub action: ConflictAction,
}

