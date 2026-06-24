use crate::{
    binder::bound::BoundSelectStmt,
    executor::{ExecutionError, ExecutionResult, Executor},
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
