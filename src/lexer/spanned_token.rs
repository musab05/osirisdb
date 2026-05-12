use crate::lexer::token::Token;

#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub start: usize,
    pub end: usize,

    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

impl Span {
    pub fn new(
        start: usize,
        end: usize,
        line: usize,
        column: usize,
    ) -> Self {
        Self {
            start,
            end,
            line,
            column,
        }
    }
}

impl SpannedToken {
    pub fn new(token: Token, span: Span) -> Self {
        Self { token, span }
    }
}