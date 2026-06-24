use crate::{
    binder::bound::BoundCreateSchemaStmt,
    common::symbol::Symbol,
    executor::{ExecutionError, ExecutionResult, Executor},
};

impl Executor {
    /// Executes a `CREATE SCHEMA` statement.
    ///
    /// Receives a fully bound and validated statement from the binder —
    /// no further validation is performed here.
    ///
    /// # Steps
    ///
    /// 1. Writes the schema entry to the catalog via `CatalogManager`
    /// 2. Creates the on-disk directory for the schema (skipped in
    ///    memory-only mode)
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
    /// bind time and execute time. If the schema already existed,
    /// `create_schema_dir` would fail with `DirectoryAlreadyExists` —
    /// but since the catalog already returned `Ok(())` for the
    /// `IF NOT EXISTS` case without inserting, we'd still attempt to
    /// create a directory that already exists on disk. To stay
    /// consistent with the database executor (which has the same
    /// property), this is treated as acceptable for now — silent
    /// success at the catalog layer takes precedence, and storage
    /// errors here are surfaced rather than swallowed.
    pub fn execute_create_schema(
        &mut self,
        db: Symbol,
        stmt: BoundCreateSchemaStmt,
    ) -> Result<ExecutionResult, ExecutionError> {
        let name = stmt.name.expect("schema name resolved by binder");

        // Resolve database and schema name strings for storage operations.
        // Storage works with &str paths, not Symbols. Resolved before the
        // catalog mutation since `create_schema` takes ownership of `stmt`.
        let db_name_str = self.catalog.interner.resolve(db).to_string();
        let schema_name_str = self.catalog.interner.resolve(name).to_string();

        // 1. Convert bound stmt → raw AST stmt for the catalog.
        self.catalog
            .create_schema(db, stmt.into(), self.session_user)
            .map_err(ExecutionError::from)?;

        // 2. Persist to system catalog.
        if let Some(sys) = &self.system_catalog {
            let entry = self
                .catalog
                .get_schema(db, name)
                .map_err(ExecutionError::from)?
                .clone();
            sys.write_schema(&entry, &mut self.catalog.interner)
                .map_err(|e| ExecutionError::Storage(e.to_string()))?;
        }

        // 3. Create on-disk directory if storage is enabled.
        //    Skipped silently in memory-only mode (tests, early pipeline).
        if let Some(storage) = &self.storage {
            storage
                .create_schema_dir(&db_name_str, &schema_name_str)
                .map_err(|e| ExecutionError::Storage(e.to_string()))?;
        }

        Ok(ExecutionResult::SchemaCreated { name })
    }
}
