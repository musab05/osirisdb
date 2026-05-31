#[derive(Debug, Clone, PartialEq)]
/// Represents the `OnCommit` SQL AST enum.
pub enum OnCommit {
    PreserveRows,
    DeleteRows,
    Drop,
}
