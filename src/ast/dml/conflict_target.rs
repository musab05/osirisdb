use crate::common::symbol::Symbol;

/// Represents the `ConflictTarget` SQL AST enum.
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictTarget {
    Columns(Vec<Symbol>),
    Constraints(Symbol),
}
