use std::{
    collections::{HashMap, hash_map::Entry},
    iter,
};

use crate::{
    catalog::{CatalogManager, system::catalog::SystemCatalog},
    common::symbol::Symbol,
    executor::ExecutionError,
    storage::{Storage, TableHeap, b_plus_tree_index::BPlusTreeIndex},
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

    /// The storage engine — manages on-disk layout.
    ///
    /// `None` when running in memory-only mode (e.g. tests that
    /// don't need disk I/O). `Some` in production.
    pub storage: Option<Storage>,

    /// Table Heap so that we dont have to repone Heap every Insert
    pub table_heaps: HashMap<(Symbol, Symbol, Symbol), TableHeap>,

    /// Table Index
    pub table_indexes: HashMap<(Symbol, Symbol, Symbol), BPlusTreeIndex>,

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
            storage: Some(storage),
            table_heaps: HashMap::new(),
            table_indexes: HashMap::new(),
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
            storage: None,
            table_heaps: HashMap::new(),
            table_indexes: HashMap::new(),
            system_catalog: None,
        }
    }

    pub fn get_or_open_table_heap(
        &mut self,
        db: Symbol,
        schema: Symbol,
        table: Symbol,
    ) -> Result<&mut TableHeap, ExecutionError> {
        match self.table_heaps.entry((db, schema, table)) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => {
                let db_name = self.catalog.interner.resolve(db);
                let schema_name = self.catalog.interner.resolve(schema);
                let table_name = self.catalog.interner.resolve(table);

                let storage = self.storage.as_ref().ok_or_else(|| {
                    ExecutionError::Storage("storage engine not initialized".to_string())
                })?;

                let heap = TableHeap::open(storage, db_name, schema_name, table_name)
                    .map_err(|e| ExecutionError::Storage(e.to_string()))?;

                Ok(entry.insert(heap))
            }
        }
    }

    pub fn get_or_open_index(
        &mut self,
        db: Symbol,
        schema: Symbol,
        index_name: Symbol,
        is_unique: bool,
    ) -> Result<&mut BPlusTreeIndex, ExecutionError> {
        match self.table_indexes.entry((db, schema, index_name)) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => {
                let db_name = self.catalog.interner.resolve(db);
                let schema_name = self.catalog.interner.resolve(schema);
                let index_name = self.catalog.interner.resolve(index_name);

                let storage = self.storage.as_ref().ok_or_else(|| {
                    ExecutionError::Storage("storage engine not initialized".to_string())
                })?;

                // Open the specific .idx file
                let index =
                    BPlusTreeIndex::open(storage, db_name, schema_name, index_name, is_unique)
                        .map_err(|e| ExecutionError::Storage(e.to_string()))?;

                Ok(entry.insert(index))
            }
        }
    }
}
