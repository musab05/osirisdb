use crate::ast::*;
use crate::common::symbol::Symbol;

/// Represents a DML statement to add new rows to a table (`INSERT INTO`).
///
/// Supports inserting raw values, loading rows from nested SELECT queries,
/// upsert operations (`ON CONFLICT`), and returning modified fields (`RETURNING`).
#[derive(Debug, Clone, PartialEq)]
pub struct InsertStmt {
    /// The qualified name of the target table.
    pub table: ObjectName,
    /// Optional list of column names targeted by the insert.
    pub columns: Vec<Symbol>,
    /// The data source containing the rows to insert (e.g. `VALUES`, `SELECT`, `DEFAULT VALUES`).
    pub source: InsertSource,
    /// Optional upsert specifications defining conflict target and action (`ON CONFLICT ...`).
    pub on_conflict: Option<OnConflict>,
    /// Optional select list items returned after successful insertion (`RETURNING`).
    pub returning: Vec<SelectItem>,
}
