use crate::{ast::CreateDatabaseStmt, common::symbol::Symbol};

/// A fully resolved and validated `CREATE DATABASE` statement.
///
/// Unlike [`CreateDatabaseStmt`] which is a raw parse-time snapshot,
/// `BoundCreateDatabaseStmt` has been validated against the catalog:
///
/// - Database name confirmed to not already exist (or IF NOT EXISTS noted)
/// - Owner resolved — guaranteed to be a valid role symbol
/// - Tablespace confirmed to exist if specified
/// - Connection limit validated to be >= -1
///
/// This is what the executor receives — no further validation needed.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundCreateDatabaseStmt {
    /// Interned name of the database to create.
    pub name: Symbol,

    /// Whether to silently succeed if the database already exists.
    pub if_not_exists: bool,

    /// Resolved owner — always present.
    ///
    /// If the user specified `OWNER alice`, this is alice's symbol.
    /// If not specified, this is the session user's symbol.
    /// Guaranteed to be a valid role in the catalog.
    pub owner: Symbol,

    /// Optional encoding symbol e.g. `UTF8`, `SQL_ASCII`.
    pub encoding: Option<Symbol>,

    /// Optional locale symbol e.g. `en_US.UTF-8`.
    pub locale: Option<Symbol>,

    /// Optional tablespace symbol.
    ///
    /// If present, confirmed to exist in the catalog at bind time.
    pub tablespace: Option<Symbol>,

    /// Optional connection limit.
    ///
    /// Validated to be >= -1. `None` means unlimited.
    pub connection_limit: Option<i64>,
}

/// Converts a bound statement back into a raw AST statement for the catalog.
///
/// The bound statement has already resolved all defaults (e.g. owner),
/// so the resulting `CreateDatabaseStmt` has `owner` set to the resolved
/// value and `if_not_exists` preserved as-is.
impl From<BoundCreateDatabaseStmt> for CreateDatabaseStmt {
    fn from(b: BoundCreateDatabaseStmt) -> Self {
        CreateDatabaseStmt {
            name: b.name,
            if_not_exists: b.if_not_exists,
            owner: Some(b.owner), // owner is always resolved in bound stmt
            encoding: b.encoding,
            locale: b.locale,
            tablespace: b.tablespace,
            connection_limit: b.connection_limit,
        }
    }
}
