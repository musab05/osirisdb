use std::{
    iter,
    sync::{Arc, Mutex},
};

use crate::{
    catalog::{CatalogManager, system::catalog::SystemCatalog},
    common::symbol::Symbol,
    executor::ExecutionError,
    storage::{Storage, TableHeap, btree::BPlusTreeIndex},
};

/// The execution engine — receives bound statements and applies them
/// to the catalog and storage.
///
/// # Responsibilities
///
/// - Calls `CatalogManager` to register metadata for DDL statements
/// - Calls the storage layer to create or modify on-disk structures
/// - Returns execution results to the caller
///
/// # What the executor does NOT do
///
/// - Validate names — that is the binder's job
/// - Parse SQL — that is the parser's job
/// - Optimize queries — that is the planner's job
///
/// # Relationship to other layers
///
/// The executor owns `CatalogManager` — it is the only layer that
/// mutates the catalog. The binder only reads the catalog via a
/// shared reference.
pub struct Executor {
    /// The catalog manager — owns the in-memory catalog and the interner.
    ///
    /// The executor is the sole writer to the catalog.
    pub catalog: CatalogManager,

    /// The current session user symbol.
    ///
    /// Used as the default owner for objects created without an explicit
    /// OWNER clause. Resolved during session initialization.
    pub session_user: Symbol,

    /// Tracks the active database namespace for this session execution context.
    /// `None` means no database has been selected yet.
    pub current_database: Option<Symbol>,

    /// The storage engine — manages on-disk layout.
    ///
    /// `None` when running in memory-only mode (e.g. tests that
    /// don't need disk I/O). `Some` in production.
    pub storage: Option<Storage>,

    /// System catalog
    pub system_catalog: Option<SystemCatalog>,
}

impl Executor {
    /// Creates a new `Executor` for the given session.
    ///
    /// `catalog` is moved into the executor — the executor takes full
    /// ownership and is the only component that may write to it.
    /// `session_user` is the symbol of the currently connected role.
    pub fn new(catalog: CatalogManager, session_user: Symbol, storage: Storage) -> Self {
        let data_dir = storage.data_dir().to_path_buf();
        let system_catalog = SystemCatalog::new(data_dir);
        system_catalog.init().ok(); // create _system/ if missing

        let mut catalog = catalog;

        // Rebuild in-memory catalog from system tables on startup.
        // If system tables are empty (first run), load_all returns an
        // empty Vec and the catalog stays empty — DDL will populate it.
        if let Ok(databses) = system_catalog.load_all(&mut catalog.interner) {
            for db_entry in databses {
                catalog.catalog.databases.insert(db_entry.name, db_entry);
            }
            // Restore the OID counter to max(existing OIDs) + 1 so new
            // objects don't collide with ones loaded from disk.
            let max_oid =
                catalog
                    .catalog
                    .databases
                    .values()
                    .flat_map(|db| {
                        iter::once(db.oid).chain(db.schemas.values().flat_map(|s| {
                            iter::once(s.oid).chain(s.tables.values().map(|t| t.oid))
                        }))
                    })
                    .max()
                    .unwrap_or(0);
            catalog.catalog.set_next_oid(max_oid + 1);
        }

        Self {
            catalog,
            session_user,
            current_database: None,
            storage: Some(storage),
            system_catalog: Some(system_catalog),
        }
    }

    /// Creates an executor without storage — catalog only.
    ///
    /// Used in tests and early pipeline stages where disk I/O
    /// is not needed. Storage operations are silently skipped.
    pub fn new_in_memory(catalog: CatalogManager, session_user: Symbol) -> Self {
        Self {
            catalog,
            session_user,
            current_database: None,
            storage: None,
            system_catalog: None,
        }
    }

    /// Returns the table's shared heap handle. Table must already exist
    /// (created via `CREATE TABLE`, which is the only place a `TableHeap`
    /// is ever opened) — this never opens a file itself.
    pub fn get_table_heap(
        &mut self,
        db: Symbol,
        schema: Symbol,
        table: Symbol,
    ) -> Result<Arc<Mutex<TableHeap>>, ExecutionError> {
        let entry = self.catalog.get_table(db, schema, table)?;
        entry
            .heap
            .clone()
            .ok_or_else(|| ExecutionError::Storage("table heap not initialized".to_string()))
    }

    /// Returns the shared index handle for `index_key` (a column name for
    /// single-column PK/UNIQUE, or a constraint name for composite ones —
    /// same key the catalog used when building `TableEntry.indexes` at
    /// `CREATE TABLE` time). Returns an error if no such index exists —
    /// callers should only call this for columns/constraints already
    /// confirmed indexed.
    pub fn get_index(
        &mut self,
        db: Symbol,
        schema: Symbol,
        table: Symbol,
        index_key: Symbol,
    ) -> Result<Arc<Mutex<BPlusTreeIndex>>, ExecutionError> {
        let entry = self.catalog.get_table(db, schema, table)?;
        entry
            .indexes
            .get(&index_key)
            .cloned()
            .ok_or_else(|| ExecutionError::Storage("index not found for column".to_string()))
    }
}
