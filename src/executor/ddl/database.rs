use crate::{
    binder::bound::database::BoundCreateDatabaseStmt,
    executor::{ExecutionResult, error::ExecutionError, executor::Executor},
};

impl Executor {
    /// Executes a `CREATE DATABASE` statement.
    ///
    /// Receives a fully bound and validated statement from the binder —
    /// no further validation is performed here.
    ///
    /// # Steps
    ///
    /// 1. Writes the database entry to the catalog via `CatalogManager`
    /// 2. Creates the on-disk directory for the database (stubbed for now)
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError::Catalog`] if the catalog rejects the operation.
    /// Returns [`ExecutionError::Storage`] if the on-disk creation fails.
    ///
    /// # Note on IF NOT EXISTS
    ///
    /// The binder already validated existence. The catalog still handles
    /// `IF NOT EXISTS` silently in case of a race condition between
    /// bind time and execute time.
    pub fn execute_create_database(
        &mut self,
        stmt: BoundCreateDatabaseStmt,
    ) -> Result<ExecutionResult, ExecutionError> {
        let name = stmt.name;

        // Resolve the database name string for storage operations.
        // Storage works with &str paths, not Symbols.
        let name_str = self.catalog.interner.resolve(name).to_string();

        // 1. Convert bound stmt → raw AST stmt for the catalog.
        // Owner is always resolved at this point — Some() guaranteed.
        self.catalog
            .create_database(stmt.into(), self.session_user)
            .map_err(ExecutionError::from)?;

        // 2. Create on-disk directory if storage is enabled.
        //    Skipped silently in memory-only mode (tests, early pipeline).
        if let Some(storage) = &self.storage {
            storage
                .create_database_dir(&name_str)
                .map_err(|e| ExecutionError::Storage(e.to_string()))?;
        }

        Ok(ExecutionResult::DatabaseCreated { name })
    }
}
