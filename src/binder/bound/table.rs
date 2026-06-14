use crate::{catalog::objects::ColumnEntry, common::symbol::Symbol};

/// Bound `CREATE TABLE` statement — resolved and validated,
/// ready for the executor to apply to the catalog and storage.
///
/// Contains the target database, schema, table name, and resolved column definitions.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundCreateTableStmt {
    /// The database in which the table will be created.
    pub db: Symbol,

    /// The schema in which the table will be created.
    pub schema: Symbol,

    /// The name of the table to create.
    pub name: Symbol,

    /// The resolved and validated column entries for the new table.
    pub columns: Vec<ColumnEntry>,

    /// If true, the statement will succeed silently if a table with the same
    /// name already exists.
    pub if_not_exists: bool,
}
