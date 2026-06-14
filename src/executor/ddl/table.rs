use crate::{
    binder::bound::BoundCreateTableStmt,
    executor::{ExecutionError, ExecutionResult, Executor},
};

impl Executor {
    /// Executes a `CREATE TABLE` statement.
    ///
    /// Receives a fully bound and validated statement from the binder —
    /// no further validation is performed here.
    ///
    /// # Steps
    ///
    /// 1. Writes the table entry (columns + table-level constraints) to
    ///    the catalog via `CatalogManager`
    /// 2. Creates the on-disk `.dat` file for the table (skipped in
    ///    memory-only mode)
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError::Catalog`] if the catalog rejects the operation.
    /// Returns [`ExecutionError::Storage`] if the on-disk creation fails.
    pub fn execute_create_table(
        &mut self,
        stmt: BoundCreateTableStmt,
    ) -> Result<ExecutionResult, ExecutionError> {
        let name = stmt.name;

        // Resolve names for storage operations before the catalog mutation
        // takes ownership of `stmt`'s fields.
        let db_name = self.catalog.interner.resolve(stmt.db).to_string();
        let schema_name = self.catalog.interner.resolve(stmt.schema).to_string();
        let table_name = self.catalog.interner.resolve(name).to_string();

        // 1. Insert table entry into the catalog.
        self.catalog
            .create_table(
                stmt.db,
                stmt.schema,
                stmt.name,
                stmt.columns,
                stmt.constraints,
                stmt.if_not_exists,
            )
            .map_err(ExecutionError::from)?;

        // 2. Create on-disk table file if storage is enabled.
        //    Skipped silently in memory-only mode (tests, early pipeline).
        if let Some(storage) = &self.storage {
            storage
                .create_table_file(&db_name, &schema_name, &table_name)
                .map_err(|e| ExecutionError::Storage(e.to_string()))?;
        }

        Ok(ExecutionResult::TableCreated { name })
    }
}
