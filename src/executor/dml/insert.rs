use crate::{
    ast::{TableConstraint, Value},
    binder::bound::BoundInsertStmt,
    catalog::objects::ColumnEntry,
    executor::{ExecutionError, ExecutionResult, Executor},
    storage::{record_id::RecordId, tuple::serialize_tuple},
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

            // 1. Ensure the main table heap is open and cached
            self.get_or_open_table_heap(db, schema, table)?;

            // Cache for tracking duplicate entries contained within this exact input batch.
            let mut pending_rows: Vec<Vec<Value>> = Vec::new();

            for row in &rows {
                // ── A. Check Single-Column Primary Key and Unique Constraints ──
                for (col_idx, col) in columns.iter().enumerate() {
                    if (col.is_primary_key || col.is_unique) && !matches!(row[col_idx], Value::Null)
                    {
                        let value_to_check = &row[col_idx];

                        // Build a single-column mini-schema to properly serialize the key bytes
                        let col_schema = vec![col.clone()];
                        let encoded_key = serialize_tuple(
                            &col_schema,
                            &[value_to_check.clone()],
                            &self.catalog.interner,
                        )
                        .map_err(|e| ExecutionError::Storage(e.to_string()))?;

                        // Dynamically look up or open the index associated with this column name
                        let index = self.get_or_open_index(db, schema, col.name, true)?;

                        // Execute O(log N) binary search on disk frame memory instead of scanning table heap
                        if index
                            .lookup(&encoded_key)
                            .map_err(|e| ExecutionError::Storage(e.to_string()))?
                            .is_some()
                        {
                            return Err(ExecutionError::Storage(format!(
                                "duplicate key value violates unique/primary key constraint for column '{}'",
                                self.catalog.interner.resolve(col.name)
                            )));
                        }

                        // Still validate against rows currently sitting in our uncommitted statement batch
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

                // ── B. Check Composite/Table-Level Unique/Primary Key Constraints ──
                for constraint in &constraints {
                    match constraint {
                        TableConstraint::PrimaryKey {
                            name,
                            columns: pk_cols,
                        }
                        | TableConstraint::Unique {
                            name,
                            columns: pk_cols,
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

                            let new_key: Vec<Value> =
                                col_indices.iter().map(|&idx| row[idx].clone()).collect();

                            // Skip if any column in the unique key is NULL (standard SQL behavior)
                            if new_key.iter().any(|v| matches!(v, Value::Null)) {
                                continue;
                            }

                            // Building the composite key constraint structural schema matching column types
                            let composite_schema: Vec<ColumnEntry> = col_indices
                                .iter()
                                .map(|&idx| columns[idx].clone())
                                .collect();

                            let encoded_composite_key = serialize_tuple(
                                &composite_schema,
                                &new_key,
                                &self.catalog.interner,
                            )
                            .map_err(|e| ExecutionError::Storage(e.to_string()))?;

                            // Fetch the index using the constraint's internal Symbol name tracking key
                            let index = self.get_or_open_index(
                                db,
                                schema,
                                name.expect("composite constraint must be named"),
                                true,
                            )?;
                            if index
                                .lookup(&encoded_composite_key)
                                .map_err(|e| ExecutionError::Storage(e.to_string()))?
                                .is_some()
                            {
                                return Err(ExecutionError::Storage(
                                    "duplicate key value violates composite unique/primary key constraint".to_string()
                                ));
                            }

                            // Check against pending rows in the same transaction batch
                            for pending_row in &pending_rows {
                                let pending_key: Vec<&Value> =
                                    col_indices.iter().map(|&idx| &pending_row[idx]).collect();
                                let current_refs: Vec<&Value> = new_key.iter().collect();
                                if current_refs == pending_key {
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

            // ── C. All Constraints Passed: Perform Persistent Writes & Index Synchronization ──
            for row in &rows {
                let table_heap = self
                    .table_heaps
                    .get_mut(&key)
                    .expect("table heap is cached");

                // Write the tuple to disk and retrieve its updated location parameters
                let (page_id, slot_id) = table_heap
                    .insert_tuple(&columns, row, &self.catalog.interner)
                    .map_err(|e| ExecutionError::Storage(e.to_string()))?;

                let new_record_id = RecordId { page_id, slot_id };

                // Update single-column indexes with the new record location coordinates
                for (col_idx, col) in columns.iter().enumerate() {
                    if col.is_primary_key || col.is_unique {
                        let col_schema = vec![col.clone()];
                        let encoded_key = serialize_tuple(
                            &col_schema,
                            &[row[col_idx].clone()],
                            &self.catalog.interner,
                        )
                        .map_err(|e| ExecutionError::Storage(e.to_string()))?;

                        let index = self.get_or_open_index(db, schema, col.name, true)?;
                        index
                            .insert(&encoded_key, new_record_id)
                            .map_err(|e| ExecutionError::Storage(e.to_string()))?;
                    }
                }

                // Update composite table-level constraints with the new record location coordinates
                for constraint in &constraints {
                    match constraint {
                        TableConstraint::PrimaryKey {
                            columns: pk_cols,
                            name,
                        }
                        | TableConstraint::Unique {
                            columns: pk_cols,
                            name,
                        } => {
                            let col_indices: Vec<usize> = pk_cols
                                .iter()
                                .map(|col_name| {
                                    columns.iter().position(|c| &c.name == col_name).unwrap()
                                })
                                .collect();

                            let composite_schema: Vec<ColumnEntry> = col_indices
                                .iter()
                                .map(|&idx| columns[idx].clone())
                                .collect();

                            let key_vals: Vec<Value> =
                                col_indices.iter().map(|&idx| row[idx].clone()).collect();
                            if key_vals.iter().any(|v| matches!(v, Value::Null)) {
                                continue;
                            }

                            let encoded_composite_key = serialize_tuple(
                                &composite_schema,
                                &key_vals,
                                &self.catalog.interner,
                            )
                            .map_err(|e| ExecutionError::Storage(e.to_string()))?;

                            let index = self.get_or_open_index(
                                db,
                                schema,
                                name.expect("composite constraint must be named"),
                                true,
                            )?;
                            index
                                .insert(&encoded_composite_key, new_record_id)
                                .map_err(|e| ExecutionError::Storage(e.to_string()))?;
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(ExecutionResult::Inserted {
            name: table,
            count: row_count,
        })
    }
}
