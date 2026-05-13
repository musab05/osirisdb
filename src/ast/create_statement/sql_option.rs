#[derive(Debug, Clone, PartialEq)]
pub struct SqlOption {
    pub name: String,
    pub value: Expr,
}