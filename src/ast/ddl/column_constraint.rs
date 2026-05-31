use crate::ast::*;

/// Represents constraints applied to a single column in a table definition.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnConstraint {
    /// Allows the column to store `NULL` values.
    Null,
    /// Prevents the column from storing `NULL` values (`NOT NULL`).
    NotNull,

    /// Specifies a default fallback value for the column when not provided in an INSERT statement (`DEFAULT <expr>`).
    Default(Expr),

    /// Enforces that all values in this column must be distinct across the table (`UNIQUE`).
    Unique,

    /// Identifies this column as the primary key of the table (`PRIMARY KEY`).
    PrimaryKey,

    /// Validates values in this column against a boolean expression before saving (`CHECK (<expr>)`).
    Check(Expr),

    /// Defines a foreign key constraint linking this column to a target table and columns (`REFERENCES table(columns)`).
    References {
        /// The referenced foreign table path.
        table: Vec<String>,
        /// The referenced columns in the foreign table.
        columns: Vec<String>,
        /// Referential action applied on deletion of a parent record (`ON DELETE`).
        on_delete: Option<ReferentialAction>,
        /// Referential action applied on modification of a parent record (`ON UPDATE`).
        on_update: Option<ReferentialAction>,
    },

    /// Automatically increment the column value (e.g. MySQL `AUTO_INCREMENT` or SQLite `AUTOINCREMENT`).
    AutoIncrement,
}
