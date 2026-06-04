use crate::lexer::token::TokenKind;

/// Returns the left and right binding power for an infix (binary) operator token,
/// or `None` if the token is not an infix operator.
///
/// Binding powers are used by the Pratt expression parser to determine operator
/// precedence and associativity. Each pair `(left_bp, right_bp)` satisfies
/// `left_bp < right_bp`, making all operators **left-associative**.
///
/// ## Precedence Table (lowest → highest)
///
/// | BP (L, R) | Operators                        |
/// |-----------|----------------------------------|
/// | (1, 2)    | `OR`                             |
/// | (3, 4)    | `AND`                            |
/// | (5, 6)    | `=`, `!=` / `<>`                 |
/// | (7, 8)    | `<`, `<=`, `>`, `>=`             |
/// | (9, 10)   | `+`, `-`                         |
/// | (11, 12)  | `*`, `/`, `%`                    |
/// | (13, 14)  | `\|\|` (string concatenation)    |
/// | (15, 16)  | `::` (PostgreSQL-style cast)     |
/// | (17, 18)  | `.` (qualified name / member)    |
pub fn infix_binding_power(token: &TokenKind) -> Option<(u8, u8)> {
    match token {
        TokenKind::Or => Some((1, 2)),
        TokenKind::And => Some((3, 4)),
        TokenKind::Eq | TokenKind::Ne => Some((5, 6)),
        TokenKind::Lt | TokenKind::Le | TokenKind::Gt | TokenKind::Ge => Some((7, 8)),
        TokenKind::Plus | TokenKind::Minus => Some((9, 10)),
        TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Some((11, 12)),
        TokenKind::Concat => Some((13, 14)),
        TokenKind::DoubleColon => Some((15, 16)), // cast
        TokenKind::Dot => Some((17, 18)),         // table.col
        _ => None,
    }
}

/// Returns the right binding power for a prefix (unary) operator token,
/// or `None` if the token is not a prefix operator.
///
/// Supported prefix operators:
/// - `NOT` — logical negation (bp 5, binds tighter than `OR`/`AND` but below comparisons)
/// - `-`   — arithmetic negation (bp 13, binds tighter than `+`/`-` but below `*`/`/`)
pub fn prefix_binding_power(token: &TokenKind) -> Option<u8> {
    match token {
        TokenKind::Not => Some(5),
        TokenKind::Minus => Some(13),
        _ => None,
    }
}
