use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    ast::TableConstraint,
    catalog::objects::column::ColumnEntry,
    common::symbol::Symbol,
    storage::{BPlusTreeIndex, TableHeap},
};

/// The catalog's runtime representation of a table.
///
/// A `TableEntry` represents a table stored in a database schema.
/// It contains a unique object identifier (OID), the table name,
/// its column definitions (with resolved constraint flags), and
/// any table-level constraints that could not be folded into a
/// single column (composite `PRIMARY KEY`/`UNIQUE`, `CHECK`, `FOREIGN KEY`).
#[derive(Clone)]
pub struct TableEntry {
    /// Internal unique identifier — never changes after creation.
    pub oid: u32,

    /// The name of the table.
    pub name: Symbol,

    /// The columns belonging to this table, with constraints resolved
    /// into per-column flags by the binder.
    pub columns: Vec<ColumnEntry>,

    /// Table-level constraints that span multiple columns or cannot be
    /// represented as a single-column flag.
    ///
    /// Includes composite `PRIMARY KEY`/`UNIQUE`, `CHECK` expressions,
    /// and `FOREIGN KEY` constraints. Referenced column names have been
    /// validated to exist on this table at bind time — cross-table
    /// validation (e.g. that a referenced foreign table/columns exist)
    /// is deferred until the catalog supports that lookup.
    pub constraints: Vec<TableConstraint>,

    pub heap: Option<Arc<Mutex<TableHeap>>>,

    pub indexes: HashMap<Symbol, Arc<Mutex<BPlusTreeIndex>>>,
}

impl TableEntry {
    /// Creates a new `TableEntry`.
    ///
    /// The `oid` is assigned by the catalog manager.
    pub fn new(
        oid: u32,
        name: Symbol,
        columns: Vec<ColumnEntry>,
        constraints: Vec<TableConstraint>,
    ) -> Self {
        Self {
            oid,
            name,
            columns,
            constraints,
            heap: None,
            indexes: HashMap::new(),
        }
    }
}

impl std::fmt::Debug for TableEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TableEntry")
            .field("oid", &self.oid)
            .field("name", &self.name)
            .field("columns", &self.columns)
            .field("constraints", &self.constraints)
            .finish()
    }
}
