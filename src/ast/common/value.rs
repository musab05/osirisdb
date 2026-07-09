use crate::common::symbol::Symbol;

/// Represents a primitive literal value in an SQL statement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    /// An integer literal (e.g., `42`, `-7`).
    Int(i64),
    /// A floating-point number literal (e.g., `3.14`, `1.0e-5`).
    Float(f64),
    /// A single-quoted string literal (e.g., `'hello'`, `'it''s fine'`).
    String(Symbol),
    /// A boolean literal (`TRUE` or `FALSE`).
    Boolean(bool),
    /// A bit string literal (e.g., `B'101010'`).
    BitString(Symbol),
    /// A hexadecimal string literal (e.g., `X'deadbeef'`).
    HexString(Symbol),
    /// The SQL `NULL` value representing missing or unknown data.
    Null,
}

impl Value {
    pub fn as_int(&self) -> i64 {
        match self {
            Value::Int(n) => *n,
            _ => panic!("not an int"),
        }
    }
    pub fn as_int_opt(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            Value::Null => None,
            _ => panic!("not an int/null"),
        }
    }
    pub fn as_sym(&self) -> crate::common::symbol::Symbol {
        match self {
            Value::String(s) => *s,
            _ => panic!("not a string"),
        }
    }
    pub fn as_sym_opt(&self) -> Option<crate::common::symbol::Symbol> {
        match self {
            Value::String(s) => Some(*s),
            Value::Null => None,
            _ => panic!("not a string/null"),
        }
    }
    pub fn as_bool(&self) -> bool {
        match self {
            Value::Boolean(b) => *b,
            _ => panic!("not a bool"),
        }
    }
}
