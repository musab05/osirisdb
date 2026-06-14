use crate::{ast::TableConstraint, catalog::objects::ColumnEntry, common::symbol::Symbol};

/// Bound `CREATE TABLE` statement — resolved and validated,
/// ready for the executor to apply to the catalog and storage.
///
/// Contains the target database, schema, table name, resolved column
/// definitions (constraints folded into flags), and any remaining
/// table-level constraints that span multiple columns or require
/// validation deferred to a later stage.
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

    /// Table-level constraints that could not be folded into a single
    /// column (composite `PRIMARY KEY`/`UNIQUE`, `CHECK`, `FOREIGN KEY`).
    ///
    /// Referenced column names have been validated to exist on this
    /// table. Cross-table validation (foreign table/columns) is deferred.
    pub constraints: Vec<TableConstraint>,

    /// If true, the statement will succeed silently if a table with the same
    /// name already exists.
    pub if_not_exists: bool,
}
