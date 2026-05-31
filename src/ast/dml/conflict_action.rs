use crate::ast::*;

/// Represents the action to take when a constraint conflict occurs during insert (`ON CONFLICT`).
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictAction {
    /// Skip the row insertion without error if a uniqueness conflict occurs (`DO NOTHING`).
    DoNothing,
    /// Update existing rows matching the conflict criteria instead of inserting (`DO UPDATE SET ...`).
    DoUpdate {
        /// The list of column assignments defining new values for matching rows.
        assignments: Vec<Assignment>,
        /// Optional filter condition specifying which rows to update on conflict.
        where_: Option<Expr>,
    },
}
