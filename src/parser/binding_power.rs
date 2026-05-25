use crate::lexer::token::TokenKind;

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

pub fn prefix_binding_power(token: &TokenKind) -> Option<u8> {
    match token {
        TokenKind::Not => Some(5),
        TokenKind::Minus => Some(13),
        _ => None,
    }
}
