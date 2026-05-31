/// Represents the `DropBehavior` SQL AST enum.
#[derive(Debug, Clone, PartialEq)]
pub enum DropBehavior {
    Cascade,
    Restrict,
}
