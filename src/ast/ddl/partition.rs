use crate::ast::*;
/// Represents the SQL `PartitionClause` structure in the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct PartitionClause {
    pub kind: PartitionKind,
    pub exprs: Vec<Expr>,
}

/// Represents the `PartitionKind` SQL AST enum.
#[derive(Debug, Clone, PartialEq)]
pub enum PartitionKind {
    Range,
    List,
    Hash,
}

