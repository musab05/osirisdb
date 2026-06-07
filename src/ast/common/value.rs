use crate::common::symbol::Symbol;

/// Represents a primitive literal value in an SQL statement.
#[derive(Debug, Clone, PartialEq)]
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
