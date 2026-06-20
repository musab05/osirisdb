use crate::{
    binder::bound::BoundInsertStmt,
    executor::{ExecutionError, ExecutionResult, Executor},
    storage::TableHeap,
};

impl Executor {
    /// Executes an `INSERT INTO` statement.
    ///
    /// Receives a fully bound statement from the binder — every row has
    /// already been validated and reordered into table-declared column
    /// order, so no further checking happens here.
    ///
    /// # Steps
    ///
    /// 1. Looks up the target table's schema (`TableEntry.columns`) from
    ///    the catalog — needed to serialize each row correctly.
    /// 2. Opens (or reuses) the table's heap file via [`TableHeap`].
    /// 3. Inserts each row in `stmt.rows`, one at a time.
    ///
    /// # Storage-disabled mode
    ///
    /// If the executor has no `Storage` (memory-only mode, e.g. tests),
    /// rows are validated against the catalog schema but not persisted
    /// anywhere — same "skip silently" convention used by the DDL
    /// executors.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError::Catalog`] if the table no longer exists
    /// in the catalog (should not happen — the binder already checked,
    /// this only fires on a race between bind time and execute time).
    /// Returns [`ExecutionError::Storage`] if opening the heap file or
    /// inserting any row fails.
    pub fn execute_insert_table(
        &mut self,
        stmt: BoundInsertStmt,
    ) -> Result<ExecutionResult, ExecutionError> {
        let db = stmt.db;
        let schema = stmt.schema;
        let table = stmt.table;
        let rows = stmt.rows;

        // Look up the table's schema — needed by serialize_tuple inside
        // TableHeap::insert_tuple. Clone the columns so we don't hold a
        // borrow on self.catalog while also borrowing self.storage below.
        let table_entry = self.catalog.get_table(db, schema, table)?;
        let columns = table_entry.columns.clone();

        let row_count = rows.len();

        // Resolve names to strings before any mutable borrow of storage,
        // same pattern used in execute_create_table.
        let db_name = self.catalog.interner.resolve(db).to_string();
        let schema_name = self.catalog.interner.resolve(schema).to_string();
        let table_name = self.catalog.interner.resolve(table).to_string();

        if let Some(storage) = &self.storage {
            let mut table_heap = TableHeap::open(storage, &db_name, &schema_name, &table_name)
                .map_err(|e| ExecutionError::Storage(e.to_string()))?;

            for row in &rows {
                table_heap
                    .insert_tuple(&columns, row, &self.catalog.interner)
                    .map_err(|e| ExecutionError::Storage(e.to_string()))?;
            }
        }

        Ok(ExecutionResult::Inserted {
            name: table,
            count: row_count,
        })
    }
}
