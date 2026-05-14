use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Value),
    Column {
        table: Option<String>,
        name: String,
    },
    BinOp {
        op: BinOpKind,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },
    UnaryOp {
        op: UnaryOpKind,
        expr: Box<Expr>,
    },
    FuncCall {
        name: String,
        args: Vec<Expr>,
    },
    IsNull {
        expr: Box<Expr>,
        negated: bool,
    },
    Between {
        expr: Box<Expr>,
        low: Box<Expr>,
        high: Box<Expr>,
        negated: bool,
    },
    InList {
        expr: Box<Expr>,
        list: Vec<Expr>,
        negated: bool,
    },
    InSubquery {
        expr: Box<Expr>,
        subq: Box<SelectStmt>,
        negated: bool,
    },
    Exists {
        subq: Box<SelectStmt>,
        negated: bool,
    },
    Case {
        operand: Option<Box<Expr>>,
        when_thens: Vec<(Expr, Expr)>,
        else_: Option<Box<Expr>>,
    },
    Cast {
        expr: Box<Expr>,
        ty: DataType,
    },
    Subquery(Box<SelectStmt>),
    Wildcard,
}

