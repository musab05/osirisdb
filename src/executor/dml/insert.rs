use crate::{
    ast::{TableConstraint, Value},
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
        let constraints = table_entry.constraints.clone();

        let row_count = rows.len();

        // Memory-only mode (no Storage configured) skips persistence
        // entirely, matching the DDL executors' convention.
        if self.storage.is_some() {
            let key = (db, schema, table);

            // Ensure the heap is open and cached.
            self.get_or_open_table_heap(db, schema, table)?;
            let table_heap = self
                .table_heaps
                .get_mut(&key)
                .expect("just inserted by get_or_open_table_heap above");

            // Scan all existing rows to check for uniqueness violations
            let existing_rows = table_heap
                .scan(&columns, &self.catalog.interner)
                .map_err(|e| ExecutionError::Storage(e.to_string()))?;

            let mut pending_rows: Vec<Vec<Value>> = Vec::new();

            for row in &rows {
                // 1. Check single-column Primary Key and Unique constraints
                for (col_idx, col) in columns.iter().enumerate() {
                    if (col.is_primary_key || col.is_unique) && !matches!(row[col_idx], Value::Null)
                    {
                        let value_to_check = &row[col_idx];

                        // Check against existing rows
                        for existing_row in &existing_rows {
                            if &existing_row[col_idx] == value_to_check {
                                return Err(ExecutionError::Storage(format!(
                                    "duplicate key value violates unique/primary key constraint for column '{}'",
                                    self.catalog.interner.resolve(col.name)
                                )));
                            }
                        }

                        // Check against pending rows in the same batch
                        for pending_row in &pending_rows {
                            if &pending_row[col_idx] == value_to_check {
                                return Err(ExecutionError::Storage(format!(
                                    "duplicate key value violates unique/primary key constraint for column '{}' (batch duplicate)",
                                    self.catalog.interner.resolve(col.name)
                                )));
                            }
                        }
                    }
                }

                // 2. Check composite/table-level unique/primary key constraints
                for constraint in &constraints {
                    match constraint {
                        TableConstraint::PrimaryKey {
                            columns: pk_cols, ..
                        }
                        | TableConstraint::Unique {
                            columns: pk_cols, ..
                        } => {
                            let col_indices: Vec<usize> = pk_cols
                                .iter()
                                .map(|col_name| {
                                    columns
                                        .iter()
                                        .position(|c| &c.name == col_name)
                                        .expect("column validated at bind time")
                                })
                                .collect();

                            let new_key: Vec<&Value> =
                                col_indices.iter().map(|&idx| &row[idx]).collect();

                            // Skip if any column in the unique key is NULL (standard SQL behavior)
                            if new_key.iter().any(|v| matches!(v, Value::Null)) {
                                continue;
                            }

                            // Check against existing rows
                            for existing_row in &existing_rows {
                                let existing_key: Vec<&Value> =
                                    col_indices.iter().map(|&idx| &existing_row[idx]).collect();
                                if new_key == existing_key {
                                    return Err(ExecutionError::Storage(
                                        "duplicate key value violates composite unique/primary key constraint".to_string()
                                    ));
                                }
                            }

                            // Check against pending rows in the same batch
                            for pending_row in &pending_rows {
                                let pending_key: Vec<&Value> =
                                    col_indices.iter().map(|&idx| &pending_row[idx]).collect();
                                if new_key == pending_key {
                                    return Err(ExecutionError::Storage(
                                        "duplicate key value violates composite unique/primary key constraint (batch duplicate)".to_string()
                                    ));
                                }
                            }
                        }
                        _ => {}
                    }
                }

                pending_rows.push(row.clone());
            }

            // 3. Perform the actual insertions since all constraints passed
            for row in &rows {
                let table_heap = self
                    .table_heaps
                    .get_mut(&key)
                    .expect("table heap is cached");

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
