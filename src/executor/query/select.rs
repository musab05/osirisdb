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

        if self.storage.is_none() {
            return Ok(ExecutionResult::Selected { rows: vec![] });
        }

        // ── Index path: WHERE col = literal on a PK/UNIQUE column ──
        if let Some(pred) = &stmt.predicate {
            let col = &columns[pred.column_idx];
            if col.is_primary_key || col.is_unique {
                let col_schema = vec![col.clone()];
                let encoded_key =
                    serialize_tuple(&col_schema, &[pred.value], &self.catalog.interner)
                        .map_err(|e| ExecutionError::Storage(e.to_string()))?;

                let index = self.get_or_open_index(db, schema, pred.column_name, true)?;
                let record_id = index
                    .lookup(&encoded_key)
                    .map_err(|e| ExecutionError::Storage(e.to_string()))?;

                let rows = match record_id {
                    None => vec![],
                    Some(rid) => {
                        self.get_or_open_table_heap(db, schema, table)?;
                        let table_heap = self
                            .table_heaps
                            .get_mut(&(db, schema, table))
                            .expect("just opened");

                        match table_heap
                            .get_tuple(rid, &columns, &self.catalog.interner)
                            .map_err(|e| ExecutionError::Storage(e.to_string()))?
                        {
                            Some(row) => vec![row],
                            None => vec![],
                        }
                    }
                };

                return Ok(ExecutionResult::Selected { rows });
            }
            // Predicate exists but column isn't indexed — fall through to
            // full scan below. No post-filter applied yet (documented gap).
        }

        // ── Full scan path (no predicate, or non-indexed column) ──
        let key = (db, schema, table);

        self.get_or_open_table_heap(db, schema, table)?;

        let table_heap = self
            .table_heaps
            .get_mut(&key)
            .expect("just inserted by get_or_open_table_heap");

        let rows = table_heap
            .scan(&columns, &mut self.catalog.interner)
            .map_err(|e| ExecutionError::Storage(e.to_string()))?;

        Ok(ExecutionResult::Selected { rows })
    }
}
