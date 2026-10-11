use crate::{
    ast::{LockStrength, WaitPolicy},
    binder::bound::BoundSelectStmt,
    executor::{ExecutionError, ExecutionResult, Executor},
    storage::{
        tuple::tuple::serialize_tuple,
        txn::{
            lock::{lock_manager::LockManager, lock_mode::RowLockMode, resource::LockResource},
            transaction::Transaction,
        },
    },
};

impl Executor {
    pub fn execute_select_table(
        &mut self,
        stmt: BoundSelectStmt,
    ) -> Result<ExecutionResult, ExecutionError> {
        self.execute_select_table_with_txn(stmt, None, None)
    }

    pub fn execute_select_table_with_txn(
        &mut self,
        stmt: BoundSelectStmt,
        mut txn: Option<&mut Transaction>,
        lock_manager: Option<&LockManager>,
    ) -> Result<ExecutionResult, ExecutionError> {
        let db = stmt.db;
        let schema = stmt.schema;
        let table = stmt.table;
        let columns = stmt.columns;
        let predicate = stmt.predicate;
        let for_locking = stmt.for_locking;

        if self.storage.is_none() {
            return Ok(ExecutionResult::Selected { rows: vec![] });
        }

        // ── Index path: WHERE col = literal on a PK/UNIQUE column ──
        if let Some(pred) = &predicate {
            let col = &columns[pred.column_idx];
            if col.is_primary_key || col.is_unique {
                let col_schema = vec![col.clone()];
                let encoded_key =
                    serialize_tuple(&col_schema, &[pred.value.clone()], &self.catalog.interner)
                        .map_err(|e| ExecutionError::Storage(e.to_string()))?;

                let index_handle = self.get_index(db, schema, table, pred.column_name)?;
                let record_id = index_handle
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .lookup(&encoded_key)
                    .map_err(|e| ExecutionError::Storage(e.to_string()))?;

                let rows = match record_id {
                    None => vec![],
                    Some(rid) => {
                        let heap_handle = self.get_table_heap(db, schema, table)?;
                        let mut heap = heap_handle.lock().unwrap_or_else(|err| err.into_inner());

                        // Row Locking for the index match
                        if let Some(ref clause) = for_locking {
                            if let (Some(t), Some(lm)) = (txn.as_mut(), lock_manager) {
                                let mode = Self::map_lock_strength(clause.strength);
                                let resource = LockResource::Row {
                                    file_id: heap.file_id(),
                                    rid,
                                };

                                match clause.wait_policy {
                                    WaitPolicy::Block => {
                                        lm.acquire(t, resource, mode)
                                            .map_err(|e| ExecutionError::Storage(e.to_string()))?;
                                    }
                                    WaitPolicy::NoWait => {
                                        let acquired = lm
                                            .try_acquire(t, resource, mode)
                                            .map_err(|e| ExecutionError::Storage(e.to_string()))?;
                                        if !acquired {
                                            return Err(ExecutionError::Storage(format!(
                                                "could not obtain lock on row ({:?}): NOWAIT specified",
                                                rid
                                            )));
                                        }
                                    }
                                    WaitPolicy::SkipLocked => {
                                        let acquired = lm
                                            .try_acquire(t, resource, mode)
                                            .map_err(|e| ExecutionError::Storage(e.to_string()))?;
                                        if !acquired {
                                            return Ok(ExecutionResult::Selected { rows: vec![] });
                                        }
                                    }
                                }

                                heap.stamp_row_lock(rid, t.txn_id)
                                    .map_err(|e| ExecutionError::Storage(e.to_string()))?;
                            }
                        }

                        let row = heap
                            .get_tuple(rid, &columns, &self.catalog.interner)
                            .map_err(|e| ExecutionError::Storage(e.to_string()))?;
                        match row {
                            Some(row) => vec![row],
                            None => vec![],
                        }
                    }
                };

                return Ok(ExecutionResult::Selected { rows });
            }
            // Predicate exists but column isn't indexed — falls through to
            // full scan below. No post-filter applied yet (documented gap:
            // this returns ALL rows, not just matching ones, until a
            // post-filter step is added).
        }

        // ── Full scan path (no predicate, or non-indexed column) ──
        let heap_handle = self.get_table_heap(db, schema, table)?;
        let mut heap = heap_handle.lock().unwrap_or_else(|e| e.into_inner());
        let candidate_rows = heap
            .scan_records(&columns, &mut self.catalog.interner)
            .map_err(|e| ExecutionError::Storage(e.to_string()))?;

        let mut rows = Vec::new();
        for (rid, values) in candidate_rows {
            if let Some(ref clause) = for_locking {
                if let (Some(t), Some(lm)) = (txn.as_mut(), lock_manager) {
                    let mode = Self::map_lock_strength(clause.strength);
                    let resource = LockResource::Row {
                        file_id: heap.file_id(),
                        rid,
                    };

                    match clause.wait_policy {
                        WaitPolicy::Block => {
                            lm.acquire(t, resource, mode)
                                .map_err(|e| ExecutionError::Storage(e.to_string()))?;
                        }
                        WaitPolicy::NoWait => {
                            let acquired = lm
                                .try_acquire(t, resource, mode)
                                .map_err(|e| ExecutionError::Storage(e.to_string()))?;
                            if !acquired {
                                return Err(ExecutionError::Storage(format!(
                                    "could not obtain lock on row ({:?}): NOWAIT specified",
                                    rid
                                )));
                            }
                        }
                        WaitPolicy::SkipLocked => {
                            let acquired = lm
                                .try_acquire(t, resource, mode)
                                .map_err(|e| ExecutionError::Storage(e.to_string()))?;
                            if !acquired {
                                continue;
                            }
                        }
                    }

                    heap.stamp_row_lock(rid, t.txn_id)
                        .map_err(|e| ExecutionError::Storage(e.to_string()))?;
                }
            }

            rows.push(values);
        }

        Ok(ExecutionResult::Selected { rows })
    }

    fn map_lock_strength(strength: LockStrength) -> RowLockMode {
        match strength {
            LockStrength::Update => RowLockMode::Update,
            LockStrength::NoKeyUpdate => RowLockMode::NoKeyUpdate,
            LockStrength::Share => RowLockMode::Share,
            LockStrength::KeyShare => RowLockMode::KeyShare,
        }
    }
}
