use crate::{ast::DataType, common::symbol::Symbol};

/// Represents a column definition in a table.
///
/// A `ColumnEntry` stores the column's name and its data type,
/// which are resolved and validated during execution or binding.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnEntry {
    /// The name of the column.
    pub name: Symbol,
    /// The SQL data type of the column.
    pub data_type: DataType,
}
