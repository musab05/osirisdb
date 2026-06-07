use crate::common::symbol::Symbol;

/// Errors that can occur during catalog operations.
///
/// Each variant carries the [`Symbol`] of the object involved so the caller
/// can resolve the name via the interner for error messages without the
/// catalog needing to own string formatting logic.
#[derive(Debug, Clone, PartialEq)]
pub enum CatalogError {
    /// `CREATE DATABASE` failed — a database with this name already exists
    /// and `IF NOT EXISTS` was not specified.
    DatabaseAlreadyExists(Symbol),

    /// An operation referenced a database that does not exist.
    DatabaseNotFound(Symbol),
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CatalogError::DatabaseAlreadyExists(sym) => {
                write!(f, "database {:?} already exists", sym)
            }
            CatalogError::DatabaseNotFound(sym) => {
                write!(f, "database {:?} does not exist", sym)
            }
        }
    }
}

impl std::error::Error for CatalogError {}