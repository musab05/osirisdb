#[derive(Debug, Clone, PartialEq)]
pub struct Assignment {
    pub column: String,
    pub value: Expr,
}