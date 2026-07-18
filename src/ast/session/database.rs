use crate::common::symbol::Symbol;

/// Represents a `USE` statement.
///
/// This AST node captures the parameters for switching the current active database,
/// corresponding to `USE name;`
#[derive(Debug, Clone, PartialEq)]
pub struct UseDatabaseStmt {
    /// The name of the database to switch to.
    pub database_name: Symbol,
}
