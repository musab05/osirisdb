/// A contiguous region of source text with line and column metadata.
///
/// Records the start and end byte indexes as well as the 1-based human-readable line
/// and column numbers for error reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// The starting byte index of this span in the source SQL.
    pub start: usize,
    /// The ending (exclusive) byte index of this span in the source SQL.
    pub end: usize,
    /// The 1-based line number of the start of this span.
    pub line: usize,
    /// The 1-based column number of the start of this span.
    pub column: usize,
}