use crate::{
    ast::schema,
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
    /// 1. Writes the table entry to the catalog via `CatalogManager`.
    /// 2. Creates the on-disk file for the table (skipped in memory-only mode).
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError::Catalog`] if the catalog rejects the operation.
    /// Returns [`ExecutionError::Storage`] if the on-disk table file creation fails.
    pub fn execute_create_table(
        &mut self,
        stmt: BoundCreateTableStmt,
    ) -> Result<ExecutionResult, ExecutionError> {
        let name = stmt.name;

        let db_name = self.catalog.interner.resolve(stmt.db).to_string();
        let schema_name = self.catalog.interner.resolve(stmt.schema).to_string();
        let table_name = self.catalog.interner.resolve(name).to_string();

        self.catalog
            .create_table(
                stmt.db,
                stmt.schema,
                stmt.name,
                stmt.columns,
                stmt.if_not_exists,
            )
            .map_err(ExecutionError::from)?;

        if let Some(storage) = &self.storage {
            storage
                .create_table_file(&db_name, &schema_name, &table_name)
                .map_err(|e| ExecutionError::Storage(e.to_string()))?;
        }
        Ok(ExecutionResult::TableCreated { name })
    }
}
