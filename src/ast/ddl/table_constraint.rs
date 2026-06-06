use crate::ast::*;
use crate::common::symbol::Symbol;

/// Enforces integrity constraints applied across one or more columns at the table level.
#[derive(Debug, Clone, PartialEq)]
pub enum TableConstraint {
    /// A table-level primary key, which can span multiple columns (composite primary key).
    PrimaryKey {
        /// Optional constraint identifier (e.g., `CONSTRAINT pk_users PRIMARY KEY (id)`).
        name: Option<Symbol>,
        /// The list of columns forming the primary key.
        columns: Vec<Symbol>,
    },

    /// A table-level unique constraint, which can span multiple columns (composite uniqueness).
    Unique {
        /// Optional constraint identifier name.
        name: Option<Symbol>,
        /// The list of columns that must have unique combinations of values.
        columns: Vec<Symbol>,
    },

    /// A table-level check constraint validating rows against a boolean expression.
    Check {
        /// Optional constraint identifier name.
        name: Option<Symbol>,
        /// The validation expression.
        expr: Expr,
    },

    /// A table-level foreign key constraint linking a group of columns to another table.
    ForeignKey {
        /// Optional constraint identifier name.
        name: Option<Symbol>,
        /// The columns in this table that refer to the foreign table.
        columns: Vec<Symbol>,
        /// The referenced foreign table path.
        foreign_table: ObjectName,
        /// The referenced columns in the foreign table.
        referred_columns: Vec<Symbol>,
        /// Referential action applied on deletion of a parent record (`ON DELETE`).
        on_delete: Option<ReferentialAction>,
        /// Referential action applied on modification of a parent record (`ON UPDATE`).
        on_update: Option<ReferentialAction>,
    },
}
