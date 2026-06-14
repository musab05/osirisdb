use crate::{
    ast::{DataType, Expr},
    common::symbol::Symbol,
};

/// Represents a column definition in a table.
///
/// A `ColumnEntry` stores the column's name, data type, and the
/// resolved per-column constraint flags. Constraints that were
/// expressed in `ColumnConstraint` form at parse time (e.g.
/// `NOT NULL`, `PRIMARY KEY`, `DEFAULT <expr>`) are folded into
/// these flags during binding — the executor and storage layers
/// never need to inspect `ColumnConstraint` directly.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnEntry {
    /// The name of the column.
    pub name: Symbol,

    /// The SQL data type of the column.
    pub data_type: DataType,

    /// Whether this column may contain `NULL` values.
    ///
    /// Defaults to `true` per the SQL standard unless `NOT NULL`
    /// or `PRIMARY KEY` was specified, either of which forces `false`.
    pub nullable: bool,

    /// Optional default value expression (`DEFAULT <expr>`).
    ///
    /// Stored but not evaluated — evaluation happens at `INSERT` time
    /// once an expression evaluator exists.
    pub default: Option<Expr>,

    /// Whether this column has a column-level `UNIQUE` constraint.
    ///
    /// Composite uniqueness across multiple columns is represented
    /// separately as a table-level [`crate::ast::TableConstraint::Unique`]
    /// on [`super::table::TableEntry::constraints`], not via this flag.
    pub is_unique: bool,

    /// Whether this column is (part of) the table's primary key.
    ///
    /// Set for a column-level `PRIMARY KEY` constraint, or for any
    /// column listed in a table-level
    /// [`crate::ast::TableConstraint::PrimaryKey`].
    pub is_primary_key: bool,
}
