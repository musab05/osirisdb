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

    /// `CREATE SCHEMA` failed - a schema with this name already exist
    /// and `IF NOT EXIST` was not specified.
    SchemaAlreadyExists(Symbol),

    /// An operation referenced a schema that does not exist.
    SchemaNotFound(Symbol),

    /// `CREATE TABLE` failed - a schema with this name already exist
    /// and `IF NOT EXIST` was not specified.
    TableAlreadyExists(Symbol),

    /// An operation referenced a table that does not exist.
    TableNotFound(Symbol),

    StorageError(String),
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
            CatalogError::SchemaAlreadyExists(sym) => {
                write!(f, "schema {:?} already exists", sym)
            }
            CatalogError::SchemaNotFound(sym) => {
                write!(f, "schema {:?} does not exist", sym)
            }
            CatalogError::TableAlreadyExists(sym) => {
                write!(f, "table {:?} already exists", sym)
            }
            CatalogError::TableNotFound(sym) => {
                write!(f, "table {:?} does not exist", sym)
            }
            CatalogError::StorageError(msg) => {
                write!(f, "storage error: {}", msg)
            }
        }
    }
}

impl CatalogError {
    pub fn format(&self, interner: &crate::common::Interner) -> String {
        match self {
            CatalogError::DatabaseAlreadyExists(sym) => {
                format!("database \"{}\" already exists", interner.resolve(*sym))
            }
            CatalogError::DatabaseNotFound(sym) => {
                format!("database \"{}\" does not exist", interner.resolve(*sym))
            }
            CatalogError::SchemaAlreadyExists(sym) => {
                format!("schema \"{}\" already exists", interner.resolve(*sym))
            }
            CatalogError::SchemaNotFound(sym) => {
                format!("schema \"{}\" does not exist", interner.resolve(*sym))
            }
            CatalogError::TableAlreadyExists(sym) => {
                format!("table \"{}\" already exists", interner.resolve(*sym))
            }
            CatalogError::TableNotFound(sym) => {
                format!("table \"{}\" does not exist", interner.resolve(*sym))
            }
            CatalogError::StorageError(msg) => {
                format!("storage error: {}", msg)
            }
        }
    }
}

impl std::error::Error for CatalogError {}
