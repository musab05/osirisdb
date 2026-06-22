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

    /// A `CREATE TABLE` referenced a schema that does not exist.
    SchemaNotFound(Symbol),

    /// A `CREATE TABLE` was attempted but the table already exists
    /// and `IF NOT EXISTS` was not specified.
    TableAlreadyExists(Symbol),

    /// A `CREATE TABLE` defined the same column name more than once.
    DuplicateColumnName(Symbol),

    /// A table-level constraint (e.g. `PRIMARY KEY (col)`, `UNIQUE (col)`,
    /// `FOREIGN KEY (col) ...`) referenced a column name that is not
    /// defined on this table.
    ColumnNotFound(Symbol),

    /// More than one `PRIMARY KEY` was specified for a table — either
    /// via multiple column-level `PRIMARY KEY` constraints, multiple
    /// table-level `PRIMARY KEY` constraints, or a combination of both.
    MultiplePrimaryKeys,

    /// A table referenced by a query does not exist in the catalog.
    TableNotFound(Symbol),

    /// The query specifies an INSERT source that is not supported.
    UnsupportedInsertSource,

    /// The query contains an expression that is not supported.
    UnsupportedExpression,

    /// The query contains an ON CONFLICT clause that is not supported.
    UnsupportedOnConflict,

    /// The query contains a RETURNING clause that is not supported.
    UnsupportedReturning,

    /// The number of target columns does not match the number of source columns.
    ColumnCountMismatch { expected: usize, found: usize },

    /// An `INSERT` left a `NOT NULL` column with no supplied value.
    ///
    /// Distinct from [`BindError::ColumnCountMismatch`] — the row had the
    /// right number of values, but a required column simply wasn't among
    /// the columns targeted by the statement.
    MissingNotNullColumn(Symbol),

    /// A `SELECT` statements feature which are not yet implemented
    /// Any feature which is not yet implemented will give error for now
    UnsupportedSelect,
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
            BindError::SchemaNotFound(s) => {
                write!(f, "schema {:?} does not exist", s)
            }
            BindError::TableAlreadyExists(s) => {
                write!(f, "table {:?} already exist", s)
            }
            BindError::DuplicateColumnName(s) => {
                write!(f, "column {:?} specified more than once", s)
            }
            BindError::ColumnNotFound(s) => {
                write!(f, "column {:?} does not exist", s)
            }
            BindError::MultiplePrimaryKeys => {
                write!(f, "multiple primary keys for table specified")
            }
            BindError::TableNotFound(s) => {
                write!(f, "table {:?} does not exist", s)
            }
            BindError::UnsupportedInsertSource => {
                write!(f, "unsupported insert source")
            }
            BindError::UnsupportedExpression => {
                write!(f, "unsupported expression")
            }
            BindError::UnsupportedOnConflict => {
                write!(f, "unsupported ON CONFLICT clause")
            }
            BindError::UnsupportedReturning => {
                write!(f, "unsupported RETURNING clause")
            }
            BindError::ColumnCountMismatch { expected, found } => {
                write!(
                    f,
                    "column count mismatch: expected {}, found {}",
                    expected, found
                )
            }
            BindError::MissingNotNullColumn(s) => {
                write!(f, "column {:?} is NOT NULL but was not given a value", s)
            }
            BindError::UnsupportedSelect => write!(f, "unsupported feature request for select"),
        }
    }
}

impl std::error::Error for BindError {}
