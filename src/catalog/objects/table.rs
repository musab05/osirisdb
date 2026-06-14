use crate::{catalog::objects::column::ColumnEntry, common::symbol::Symbol};

/// The catalog's runtime representation of a table.
///
/// A `TableEntry` represents a table stored in a database schema.
/// It contains a unique object identifier (OID), the table name,
/// and the definitions of its columns.
#[derive(Debug, Clone, PartialEq)]
pub struct TableEntry {
    /// Internal unique identifier — never changes after creation.
    pub oid: u32,

    /// The name of the table.
    pub name: Symbol,

    /// The columns belonging to this table.
    pub columns: Vec<ColumnEntry>,
}

impl TableEntry {
    /// Creates a new `TableEntry`.
    ///
    /// The `oid` is assigned by the catalog manager.
    pub fn new(oid: u32, name: Symbol, columns: Vec<ColumnEntry>) -> Self {
        Self { oid, name, columns }
    }
}
