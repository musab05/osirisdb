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

pub enum UnaryOpKind {
    Not,
    Plus,
    Minus,
}