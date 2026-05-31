/// Represents the `ReferentialAction` SQL AST enum.
#[derive(Debug, Clone, PartialEq)]
pub enum ReferentialAction {
    Cascade,
    Restrict,
    NoAction,
    SetNull,
    SetDefault,
}
