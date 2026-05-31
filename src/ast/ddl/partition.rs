use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
/// Represents the SQL `PartitionClause` structure in the AST.
pub struct PartitionClause {
    pub kind: PartitionKind,
    pub exprs: Vec<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
/// Represents the `PartitionKind` SQL AST enum.
pub enum PartitionKind {
    Range,
    List,
    Hash,
}

