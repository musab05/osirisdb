#[derive(Debug, Clone, PartialEq)]
pub enum ConflictTarget {
    Columns(Vec<String>),
    Constraints(String),
}

