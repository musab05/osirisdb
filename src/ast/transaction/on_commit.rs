/// Represents the `OnCommit` SQL AST enum.
#[derive(Debug, Clone, PartialEq)]
pub enum OnCommit {
    PreserveRows,
    DeleteRows,
    Drop,
}
