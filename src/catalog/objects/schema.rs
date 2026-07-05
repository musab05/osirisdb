use std::collections::HashMap;

use crate::{catalog::objects::TableEntry, common::symbol::Symbol};

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
#[derive(Clone)]
pub struct SchemaEntry {
    /// Internal unique identifier — never changes after creation.
    pub oid: u32,

    /// The name of the schema.
    pub name: Symbol,

    /// The role that owns this schema — always resolved, never None.
    pub owner: Symbol,

    /// The database this schema belongs to.
    pub database: Symbol,

    /// the tables which belongs to this schema
    pub tables: HashMap<Symbol, TableEntry>,
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
            tables: HashMap::new(),
        }
    }
}

impl std::fmt::Debug for SchemaEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SchemaEntry")
            .field("oid", &self.oid)
            .field("name", &self.name)
            .field("owner", &self.owner)
            .field("database", &self.database)
            .field("tables", &self.tables)
            .finish()
    }
}
