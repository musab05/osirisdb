use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::ast::TableConstraint;
use crate::storage::{BPlusTreeIndex, Storage, TableHeap};
use crate::{
    catalog::{
        error::CatalogError,
        manager::CatalogManager,
        objects::{ColumnEntry, TableEntry},
    },
    common::symbol::Symbol,
};

impl CatalogManager {
    /// Inserts a new table into the given database and schema.
    ///
    /// # Behavior
    ///
    /// - Database or schema not found → corresponding `*NotFound` error
    /// - Table already exists + `if_not_exists` → silent success
    /// - Table already exists + not `if_not_exists` → `TableAlreadyExists`
    pub fn create_table(
        &mut self,
        storage: &Storage,
        db: Symbol,
        schema: Symbol,
        name: Symbol,
        columns: Vec<ColumnEntry>,
        constraints: Vec<TableConstraint>,
        if_not_exists: bool,
    ) -> Result<(), CatalogError> {
        let db_entry = self
            .catalog
            .databases
            .get(&db)
            .ok_or(CatalogError::DatabaseNotFound(db))?;
        let schema_entry = db_entry
            .schemas
            .get(&schema)
            .ok_or(CatalogError::SchemaNotFound(schema))?;

        if schema_entry.tables.contains_key(&name) {
            if if_not_exists {
                return Ok(());
            }
            return Err(CatalogError::TableAlreadyExists(name));
        }

        // get OID before borrowing mutably — avoids double mutable borrow
        let oid = self.catalog.next_oid();

        let db_name = self.interner.resolve(db);
        let schema_name = self.interner.resolve(schema);
        let table_name = self.interner.resolve(name);

        // Table heap — one BufferPool over table.dat. This is the ONLY
        // place a TableHeap for this table is ever opened; the executor
        // must borrow this handle, never open its own.
        let heap = TableHeap::open(storage, &db_name, &schema_name, &table_name)
            .map_err(|e| CatalogError::StorageError(e.to_string()))?;

        let heap = Arc::new(Mutex::new(heap));

        let mut indexes: HashMap<Symbol, Arc<Mutex<BPlusTreeIndex>>> = HashMap::new();

        // Single-column PK/UNIQUE — each gets its OWN file + OWN pool.
        // Never shares the heap's pool (different physical files).
        // Keyed by column name symbol — matches how the executor already
        // looks these up (col.name), no separate naming scheme needed.
        for col in &columns {
            if col.is_primary_key || col.is_unique {
                let col_name_str = self.interner.resolve(col.name);
                let file_stem = format!("{}_{}", table_name, col_name_str);
                let index = BPlusTreeIndex::open_standalone(
                    storage,
                    &db_name,
                    &schema_name,
                    &file_stem,
                    true,
                )
                .map_err(|e| CatalogError::StorageError(e.to_string()))?;
                indexes.insert(col.name, Arc::new(Mutex::new(index)));
            }
        }

        // Composite PK/UNIQUE — keyed by constraint name symbol (binder
        // guarantees this is always Some by the time it gets here).
        for constraint in &constraints {
            if let TableConstraint::PrimaryKey {
                name: Some(cname), ..
            }
            | TableConstraint::Unique {
                name: Some(cname), ..
            } = constraint
            {
                let cname_str = self.interner.resolve(*cname);
                let file_stem = format!("{}_{}", table_name, cname_str);
                let index = BPlusTreeIndex::open_standalone(
                    storage,
                    &db_name,
                    &schema_name,
                    &file_stem,
                    true,
                )
                .map_err(|e| CatalogError::StorageError(e.to_string()))?;
                indexes.insert(*cname, Arc::new(Mutex::new(index)));
            }
        }

        let db_entry = self.catalog.databases.get_mut(&db).unwrap();
        let schema_entry = db_entry.schemas.get_mut(&schema).unwrap();
        let mut entry = TableEntry::new(oid, name, columns, constraints);
        entry.heap = Some(heap);
        entry.indexes = indexes;
        schema_entry.tables.insert(name, entry);
        Ok(())
    }

    /// Returns `true` if a table with the given name exists in the
    /// given database and schema.
    pub fn table_exists(&self, db: Symbol, schema: Symbol, name: Symbol) -> bool {
        self.catalog
            .databases
            .get(&db)
            .and_then(|d| d.schemas.get(&schema))
            .map(|s| s.tables.contains_key(&name))
            .unwrap_or(false)
    }

    /// Looks up a table by database, schema, and name.
    ///
    /// Returns [`CatalogError::DatabaseNotFound`] / [`CatalogError::SchemaNotFound`]
    /// if the database or schema does not exist, or
    /// [`CatalogError::TableNotFound`] if the table does not exist.
    pub fn get_table(
        &self,
        db: Symbol,
        schema: Symbol,
        name: Symbol,
    ) -> Result<&TableEntry, CatalogError> {
        let db_entry = self
            .catalog
            .databases
            .get(&db)
            .ok_or(CatalogError::DatabaseNotFound(db))?;
        let schema_entry = db_entry
            .schemas
            .get(&schema)
            .ok_or(CatalogError::SchemaNotFound(schema))?;
        schema_entry
            .tables
            .get(&name)
            .ok_or(CatalogError::TableNotFound(name))
    }
}
