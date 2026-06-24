use crate::ast::*;
use crate::common::symbol::Symbol;

/// Represents a table or subquery reference in the `FROM` or `JOIN` clauses of a query.
#[derive(Debug, Clone, PartialEq)]
pub enum TableRef {
    /// A named table or view, optionally qualified (e.g. `users`, `public.users`).
    Named {
        /// The qualified table path parts.
        name: ObjectName,
        /// Optional table alias identifier (e.g. `users AS u`).
        alias: Option<Symbol>,
    },

    /// A nested subquery evaluating to a table-like result (e.g., `(SELECT * FROM orders) AS o`).
    Subquery {
        /// The nested SELECT query.
        query: Box<SelectStmt>,
        /// Optional table alias identifier.
        alias: Option<Symbol>,
    },
}
