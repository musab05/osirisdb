#[derive(Debug, Clone, PartialEq)]
/// Represents the `ReferentialAction` SQL AST enum.
pub enum ReferentialAction {
    Cascade,
    Restrict,
    NoAction,
    SetNull,
    SetDefault,
}
