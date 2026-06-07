use crate::lexer::spanned_token::Span;

/// An error produced during SQL parsing, carrying a human-readable message and the
/// source span where the error was detected.
///
/// `ParserError` is returned whenever the token stream does not conform to the
/// expected SQL grammar. The attached [`Span`] enables downstream consumers
/// (editors, CLI tools) to highlight the exact location of the problem.
#[derive(Debug, Clone, PartialEq)]
pub struct ParserError {
    /// Human-readable description of what went wrong (e.g. "Expected `,`, found `)`").
    pub message: String,
    /// Byte-offset span in the original source pointing to the offending token.
    pub span: Span,
}

impl ParserError {
    /// Creates a new `ParserError` from a message and the source span of the problematic token.
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }
}

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Parse error: {} at line {}, col {}",
            self.message, self.span.line, self.span.column
        )
    }
}

impl std::error::Error for ParserError {}
