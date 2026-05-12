#[derive(Debug, Clone, PartialEq)]
pub struct SelectStmt {
    pub modifier: Option<SelectModifier>,
    pub columns: Vec<SelectItem>,
    pub from: Vec<TableRef>,
    pub joins: Vec<JoinClause>,
    pub where_: Option<Expr>,
    pub group_by: Vec<Expr>,
    pub having: Option<Expr>,
    pub order_by: Vec<OrderItem>,
    pub limit: Option<Expr>,
    pub offset: Option<Expr>,
    pub ctes: Vec<Cte>,
    pub set_op: Option<Box<SetOperation>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectModifier {
    Distinct,
    DistinctOn(Vec<Expr>),
    All,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectItem {
    Wildcard,
    QualifiedWildcard(Vec<String>),
    Expr { expr: Expr, alias: Option<String> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetOperation {
    pub op: SetOp,
    pub all: bool,
    pub right: Box<SelectStmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SetOp {
    Union,
    Intersect,
    Except,
}
