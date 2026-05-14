use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(String),
    SingleQuotedString(String),
    Boolean(bool),
    Null,
}

