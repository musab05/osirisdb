use crate::{
    binder::bound::BoundSelectStmt,
    executor::{ExecutionError, ExecutionResult, Executor},
    storage::tuple::serialize_tuple,
};

impl Executor {
    pub fn execute_select_table(
        &mut self,
        stmt: BoundSelectStmt,
    ) -> Result<ExecutionResult, ExecutionError> {
        let db = stmt.db;
        let schema = stmt.schema;
        let table = stmt.table;
        let columns = stmt.columns;
        let predicate = stmt.predicate;

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
                        let row = heap_handle
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
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
        let rows = heap_handle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .scan(&columns, &mut self.catalog.interner)
            .map_err(|e| ExecutionError::Storage(e.to_string()))?;

        Ok(ExecutionResult::Selected { rows })
    }
}
