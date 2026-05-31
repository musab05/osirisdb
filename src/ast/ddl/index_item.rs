use crate::ast::*;

/// Represents a single column key or expression key inside a `CREATE INDEX` specification.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexItem {
    /// The column reference or expression key.
    pub expr: Expr,
    /// Optional index sort criteria (`ASC` / `DESC`).
    pub order: Option<Order>,
    /// Optional null values placement preference inside the index structure (`NULLS FIRST` / `NULLS LAST`).
    pub nulls: Option<NullOrdering>,
}
