#[derive(Debug, Clone, PartialEq)]
/// Represents the `ConflictTarget` SQL AST enum.
pub enum ConflictTarget {
    Columns(Vec<String>),
    Constraints(String),
}

