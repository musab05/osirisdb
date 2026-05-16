use crate::lexer::token::Token;

pub fn infix_binding_power(token: &Token) -> Option<(u8, u8)> {
    match token {
        Token::Or => Some((1, 2)),
        Token::And => Some((3, 4)),
        Token::Eq | Token::Ne => Some((5, 6)),
        Token::Lt | Token::Le | Token::Gt | Token::Ge => Some((7, 8)),
        Token::Plus | Token::Minus => Some((9, 10)),
        Token::Star | Token::Slash | Token::Percent => Some((11, 12)),
        Token::Concat => Some((13, 14)),
        Token::DoubleColon => Some((15, 16)), // cast
        Token::Dot => Some((17, 18)),         // table.col
        _ => None,
    }
}

pub fn prefix_binding_power(token: &Token) -> Option<u8> {
    match token {
        Token::Not => Some(5),
        Token::Minus => Some(13),
        _ => None,
    }
}
