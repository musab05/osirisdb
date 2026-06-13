use crate::common::symbol::Symbol;

/// The catalog's runtime representation of a schema.
///
/// Unlike [`CreateSchemaStmt`] which is a parse-time snapshot,
/// `SchemaEntry` is what lives in the catalog after validation
/// and execution. Schemas are always nested inside a database.
///
/// Key differences from the AST node:
/// - No `if_not_exists` — consumed during execution
/// - `owner` is required — resolved from authorization or session user
/// - Adds `oid` — stable internal identifier
#[derive(Debug, Clone, PartialEq)]
pub struct SchemaEntry {
    /// Internal unique identifier — never changes after creation.
    pub oid: u32,

    /// The name of the schema.
    pub name: Symbol,

    /// The role that owns this schema — always resolved, never None.
    pub owner: Symbol,

    /// The database this schema belongs to.
    pub database: Symbol,
}

impl SchemaEntry {
    /// Creates a new `SchemaEntry`.
    ///
    /// `oid` assigned by catalog manager.
    /// `owner` is always resolved before this is called.
    pub fn new(oid: u32, name: Symbol, owner: Symbol, database: Symbol) -> Self {
        Self {
            oid,
            name,
            owner,
            database,
        }
    }
}
