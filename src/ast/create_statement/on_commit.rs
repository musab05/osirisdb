#[derive(Debug, Clone, PartialEq)]
pub enum OnCommit {
    PreserveRows,
    DeleteRows,
    Drop,
}