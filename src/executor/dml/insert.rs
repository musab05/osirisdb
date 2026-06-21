use crate::{
    binder::bound::BoundInsertStmt,
    executor::{ExecutionError, ExecutionResult, Executor},
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
    /// 2. Gets (or opens and caches) the table's heap via
    ///    [`Executor::get_or_open_table_heap`] — subsequent INSERTs into
    ///    the same table reuse the same open heap file and buffer pool
    ///    instead of reopening from disk every time.
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
        // TableHeap::insert_tuple. Clone the columns so this doesn't hold
        // a borrow on self.catalog while self.table_heaps is borrowed
        // mutably below.
        let table_entry = self.catalog.get_table(db, schema, table)?;
        let columns = table_entry.columns.clone();

        let row_count = rows.len();

        // Memory-only mode (no Storage configured) skips persistence
        // entirely, matching the DDL executors' convention.
        if self.storage.is_some() {
            let key = (db, schema, table);

            // Ensure the heap is open and cached. The returned &mut
            // TableHeap is intentionally not kept — holding it across the
            // loop below would conflict with the &self.catalog.interner
            // borrow each insert_tuple call needs. Instead each iteration
            // re-fetches it fresh from the map, which is a cheap HashMap
            // lookup, not a disk reopen.
            self.get_or_open_table_heap(db, schema, table)?;

            for row in &rows {
                let table_heap = self
                    .table_heaps
                    .get_mut(&key)
                    .expect("just inserted by get_or_open_table_heap above");

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
