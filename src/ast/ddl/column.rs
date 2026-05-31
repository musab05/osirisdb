use crate::ast::*;

/// Represents the definition of a table column (e.g. `username VARCHAR(255) NOT NULL UNIQUE`).
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    /// The name of the column.
    pub name: String,
    /// The data type of the column (e.g. `INT`, `VARCHAR(255)`).
    pub data_type: DataType,
    /// Optional collation name (e.g., `COLLATE "fr_FR"`).
    pub collation: Option<String>,
    /// Column-level constraints applied directly to this column (e.g., `NOT NULL`, `DEFAULT 0`).
    pub constraints: Vec<ColumnConstraint>,
    /// Optional generated column metadata (e.g., `GENERATED ALWAYS AS (age * 2) STORED`).
    pub generated: Option<GeneratedColumn>,
}
