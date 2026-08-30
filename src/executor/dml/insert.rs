use crate::{
    ast::{TableConstraint, Value},
    binder::bound::BoundInsertStmt,
    catalog::objects::ColumnEntry,
    common::symbol::Symbol,
    executor::{ExecutionError, ExecutionResult, Executor},
    storage::tuple::{record_id::RecordId, tuple::serialize_tuple},
};

/// Encoded index keys computed once during validation, reused during
/// the write pass so we don't re-serialize the same values twice.
struct PendingInsert {
    row: Vec<Value>,

    // (col_idx, col_name, key_bytes)
    single_col_keys: Vec<(usize, Symbol, Vec<u8>)>,

    // (constraint_name, key_bytes)
    composite_keys: Vec<(Symbol, Vec<u8>)>,
}

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

        let table_entry = self.catalog.get_table(db, schema, table)?;
        let columns = table_entry.columns.clone();
        let constraints = table_entry.constraints.clone();

        let row_count = rows.len();

        if self.storage.is_some() {
            self.get_table_heap(db, schema, table)?;

            // ── Pass A: validate + build encoded keys once ──
            let mut prepared: Vec<PendingInsert> = Vec::with_capacity(rows.len());

            for row in &rows {
                let mut single_col_keys = Vec::new();

                for (col_idx, col) in columns.iter().enumerate() {
                    if (col.is_primary_key || col.is_unique) && !matches!(row[col_idx], Value::Null)
                    {
                        let col_schema = vec![col.clone()];
                        let encoded_key = serialize_tuple(
                            &col_schema,
                            &[row[col_idx].clone()],
                            &self.catalog.interner,
                        )
                        .map_err(|e| ExecutionError::Storage(e.to_string()))?;

                        let index_handle = self.get_index(db, schema, table, col.name)?;
                        if index_handle
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .lookup(&encoded_key)
                            .map_err(|e| ExecutionError::Storage(e.to_string()))?
                            .is_some()
                        {
                            return Err(ExecutionError::Storage(format!(
                                "duplicate key value violates unique/primary key constraint for column '{}'",
                                self.catalog.interner.resolve(col.name)
                            )));
                        }

                        for prev in &prepared {
                            if prev
                                .single_col_keys
                                .iter()
                                .any(|(idx, _, k)| *idx == col_idx && k == &encoded_key)
                            {
                                return Err(ExecutionError::Storage(format!(
                                    "duplicate key value violates unique/primary key constraint for column '{}' (batch duplicate)",
                                    self.catalog.interner.resolve(col.name)
                                )));
                            }
                        }

                        single_col_keys.push((col_idx, col.name, encoded_key));
                    }
                }

                let mut composite_keys = Vec::new();

                for constraint in &constraints {
                    if let TableConstraint::PrimaryKey {
                        name,
                        columns: pk_cols,
                    }
                    | TableConstraint::Unique {
                        name,
                        columns: pk_cols,
                    } = constraint
                    {
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

                        if new_key.iter().any(|v| matches!(v, Value::Null)) {
                            continue;
                        }

                        let composite_schema: Vec<ColumnEntry> = col_indices
                            .iter()
                            .map(|&idx| columns[idx].clone())
                            .collect();

                        let encoded_composite_key =
                            serialize_tuple(&composite_schema, &new_key, &self.catalog.interner)
                                .map_err(|e| ExecutionError::Storage(e.to_string()))?;

                        let idx_name = name.expect("composite constraint must be named");
                        let index_handle = self.get_index(db, schema, table, idx_name)?;

                        if index_handle
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .lookup(&encoded_composite_key)
                            .map_err(|e| ExecutionError::Storage(e.to_string()))?
                            .is_some()
                        {
                            return Err(ExecutionError::Storage(
                            "duplicate key value violates composite unique/primary key constraint".to_string(),
                        ));
                        }

                        for prev in &prepared {
                            if prev
                                .composite_keys
                                .iter()
                                .any(|(n, k)| *n == idx_name && k == &encoded_composite_key)
                            {
                                return Err(ExecutionError::Storage(
                                "duplicate key value violates composite unique/primary key constraint (batch duplicate)".to_string(),
                            ));
                            }
                        }

                        composite_keys.push((idx_name, encoded_composite_key));
                    }
                }

                prepared.push(PendingInsert {
                    row: row.clone(),
                    single_col_keys,
                    composite_keys,
                });
            }

            let heap_handle = self.get_table_heap(db, schema, table)?;

            // ── Pass B: write heap + indexes, reusing keys computed above ──
            for pending in &prepared {
                let (page_id, slot_id) = {
                    let mut heap = heap_handle.lock().unwrap_or_else(|e| e.into_inner());
                    heap.insert_tuple(&columns, &pending.row, &self.catalog.interner, None, None)
                        .map_err(|e| ExecutionError::Storage(e.to_string()))?
                };

                let new_record_id = RecordId { page_id, slot_id };

                for (_col_idx, col_name, encoded_key) in &pending.single_col_keys {
                    let index_handle = self.get_index(db, schema, table, *col_name)?;
                    index_handle
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(encoded_key, new_record_id)
                        .map_err(|e| ExecutionError::Storage(e.to_string()))?;
                }

                for (idx_name, encoded_key) in &pending.composite_keys {
                    let index_handle = self.get_index(db, schema, table, *idx_name)?;
                    index_handle
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .insert(encoded_key, new_record_id)
                        .map_err(|e| ExecutionError::Storage(e.to_string()))?;
                }
            }
        }

        Ok(ExecutionResult::Inserted {
            name: table,
            count: row_count,
        })
    }
}
