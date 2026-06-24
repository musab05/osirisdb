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
    /// 3. Persists metadata to the system catalog
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
        let db_sym = stmt.db;
        let schema_sym = stmt.schema;

        // Resolve names before catalog mutation takes ownership of stmt.
        let db_name = self.catalog.interner.resolve(db_sym).to_string();
        let schema_name = self.catalog.interner.resolve(schema_sym).to_string();
        let table_name = self.catalog.interner.resolve(name).to_string();

        // 1. Insert table entry into the catalog.
        self.catalog
            .create_table(
                db_sym,
                schema_sym,
                stmt.name,
                stmt.columns,
                stmt.constraints,
                stmt.if_not_exists,
            )
            .map_err(ExecutionError::from)?;

        // 2. Create on-disk table file if storage is enabled.
        if let Some(storage) = &self.storage {
            storage
                .create_table_file(&db_name, &schema_name, &table_name)
                .map_err(|e| ExecutionError::Storage(e.to_string()))?;
        }

        // 3. Persist to system catalog.
        if let Some(sys) = &self.system_catalog {
            let entry = self
                .catalog
                .get_table(db_sym, schema_sym, name)
                .map_err(ExecutionError::from)?
                .clone();
            sys.write_table(&db_name, &schema_name, &entry, &mut self.catalog.interner)
                .map_err(|e| ExecutionError::Storage(e.to_string()))?;
        }

        Ok(ExecutionResult::TableCreated { name })
    }
}
