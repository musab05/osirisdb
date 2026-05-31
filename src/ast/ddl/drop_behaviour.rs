#[derive(Debug, Clone, PartialEq)]
/// Represents the `DropBehavior` SQL AST enum.
pub enum DropBehavior {
    Cascade,
    Restrict,
}
