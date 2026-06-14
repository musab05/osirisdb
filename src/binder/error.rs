use crate::common::symbol::Symbol;

/// Errors produced during the binding phase.
///
/// Binding validates names against the catalog and resolves references.
/// All errors carry [`Symbol`]s — resolve via the interner for display.
#[derive(Debug, Clone, PartialEq)]
pub enum BindError {
    /// A `CREATE DATABASE` was attempted but the database already exists
    /// and `IF NOT EXISTS` was not specified.
    DatabaseAlreadyExists(Symbol),

    /// The role specified in `OWNER` does not exist in the catalog.
    RoleNotFound(Symbol),

    /// The tablespace specified in `TABLESPACE` does not exist in the catalog.
    TablespaceNotFound(Symbol),

    /// `CONNECTION LIMIT` was given a value below -1.
    InvalidConnectionLimit(i64),

    /// A `CREATE SCHEMA` was attempted but the schema already exists
    /// and `IF NOT EXISTS` was not specified.
    SchemaAlreadyExists(Symbol),

    /// A `CREATE SCHEMA` referenced a database that does not exist.
    DatabaseNotFound(Symbol),
}

impl std::fmt::Display for BindError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BindError::DatabaseAlreadyExists(s) => {
                write!(f, "database {:?} already exists", s)
            }
            BindError::RoleNotFound(s) => {
                write!(f, "role {:?} does not exist", s)
            }
            BindError::TablespaceNotFound(s) => {
                write!(f, "tablespace {:?} does not exist", s)
            }
            BindError::InvalidConnectionLimit(n) => {
                write!(f, "invalid connection limit {}, must be >= -1", n)
            }
            BindError::SchemaAlreadyExists(s) => {
                write!(f, "schema {:?} already exist", s)
            }
            BindError::DatabaseNotFound(s) => {
                write!(f, "database {:?} does not exist", s)
            }
        }
    }
}

impl std::error::Error for BindError {}