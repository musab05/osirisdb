use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub struct PartitionClause {
    pub kind: PartitionKind,
    pub exprs: Vec<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PartitionKind {
    Range,
    List,
    Hash,
}

