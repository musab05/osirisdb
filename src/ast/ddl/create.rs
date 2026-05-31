use crate::ast::*;

/// Represents a DDL statement to construct a new table (`CREATE TABLE`).
///
/// Supports standard and advanced PostgreSQL features such as temporary tables,
/// table inheritance, partitioning, constraint specifications, custom tablespaces,
/// storage options (`WITH`), and creation from query results (`CREATE TABLE AS SELECT`).
#[derive(Debug, Clone, PartialEq)]
pub struct CreateStmt {
    /// Skip error creation if the table already exists (`IF NOT EXISTS`).
    pub if_not_exist: bool,
    /// If `true`, the table is temporary to the session (`TEMPORARY` or `TEMP`).
    pub temporary: bool,
    /// If `true`, database writes bypass write-ahead logging (PostgreSQL `UNLOGGED`).
    pub unlogged: bool,

    /// The qualified name of the table to create.
    pub name: Vec<String>,

    /// The list of column definitions specifying names, data types, and column constraints.
    pub columns: Vec<ColumnDef>,
    /// The list of table-level constraints (e.g. composite primary keys, check constraints).
    pub constraints: Vec<TableConstraint>,

    /// A list of parent tables this table inherits columns from (PostgreSQL `INHERITS`).
    pub inherits: Vec<Vec<String>>,
    /// Table partitioning specification (PostgreSQL `PARTITION BY`).
    pub partitions: Vec<PartitionClause>,

    /// Storage parameters or parameters of a table (PostgreSQL `WITH (fillfactor = 70, ...)`).
    pub with_options: Vec<SqlOption>,

    /// Optional target storage tablespace name (PostgreSQL `TABLESPACE name`).
    pub table_space: Option<String>,

    /// Action defining what happens to temporary tables when a transaction commits (`ON COMMIT`).
    pub on_commit: Option<OnCommit>,

    /// Optional select query to load initial rows (`CREATE TABLE AS SELECT ...`).
    pub as_query: Option<SelectStmt>,
}
