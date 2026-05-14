use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub enum BinOpKind {
        Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    Add,
    Sub,
    Mul,
    Div,
    Mod,

    And,
    Or,

    Like,
    In,
    Between,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOpKind {
    Not,
    Plus,
    Minus,
}
