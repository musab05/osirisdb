use crate::storage::{Storage, StorageError};

impl Storage {
    /// Creates the on-disk directory for a new schema.
    ///
    /// # Directory layout created
    ///
    /// ```text
    /// data_dir/
    ///   {db_name}/
    ///     {schema_name}/   ← created here
    /// ```
    ///
    /// # Errors
    ///
    /// - [`StorageError::DirectoryNotFound`] if the parent database
    ///   directory does not exist. The binder/catalog should have
    ///   already validated the database exists — this indicates the
    ///   database directory and catalog entry have gone out of sync.
    /// - [`StorageError::DirectoryAlreadyExists`] if the schema
    ///   directory already exists. Should not happen in normal
    ///   operation since the binder validates existence before
    ///   execution — indicates a race condition or manual filesystem
    ///   interference.
    /// - [`StorageError::Io`] if the filesystem operation fails.
    pub fn create_schema_dir(&self, db_name: &str, schema_name: &str) -> Result<(), StorageError> {
        let db_path = self.database_path(db_name);

        if !db_path.exists() {
            return Err(StorageError::DirectoryNotFound(db_path));
        }

        let schema_path = self.schema_path(db_name, schema_name);

        if schema_path.exists() {
            return Err(StorageError::DirectoryAlreadyExists(schema_path));
        }

        std::fs::create_dir(&schema_path).map_err(|e| StorageError::io(&schema_path, e))?;

        Ok(())
    }

    /// Removes the on-disk directory for a schema and all its contents.
    ///
    /// # Warning
    ///
    /// Irreversible. Caller must ensure no active references to tables
    /// in this schema exist before calling.
    ///
    /// # Errors
    ///
    /// - [`StorageError::DirectoryNotFound`] if the schema directory
    ///   does not exist.
    /// - [`StorageError::Io`] if the removal fails.
    pub fn drop_schema_dir(&self, db_name: &str, schema_name: &str) -> Result<(), StorageError> {
        let schema_path = self.schema_path(db_name, schema_name);

        if !schema_path.exists() {
            return Err(StorageError::DirectoryNotFound(schema_path));
        }

        std::fs::remove_dir_all(&schema_path).map_err(|e| StorageError::io(&schema_path, e))?;

        Ok(())
    }
}
