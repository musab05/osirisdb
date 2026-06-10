use crate::storage::{error::StorageError, storage::Storage};

impl Storage {
    /// Creates the on-disk directory structure for a new database.
    ///
    /// # Directory layout created
    ///
    /// ```text
    /// data_dir/
    ///   {db_name}/         ← database root directory
    ///     public/          ← default schema directory
    /// ```
    ///
    /// The `public` schema directory is always created alongside the
    /// database since every database has a default public schema.
    ///
    /// # Errors
    ///
    /// - [`StorageError::DirectoryAlreadyExists`] if the database
    ///   directory already exists. This should not happen in normal
    ///   operation since the binder validates existence before execution.
    ///   If it does happen it indicates a race condition or manual
    ///   filesystem interference.
    /// - [`StorageError::Io`] if any filesystem operation fails.
    pub fn create_database_dir(&self, db_name: &str) -> Result<(), StorageError> {
        let db_path = self.database_path(db_name);

        // Guard against race conditions — directory should not exist
        if db_path.exists() {
            return Err(StorageError::DirectoryAlreadyExists(db_path));
        }

        // Create the database root directory
        std::fs::create_dir(&db_path).map_err(|e| StorageError::io(&db_path, e))?;

        // Create the default public schema directory
        let public_path = db_path.join("public");
        std::fs::create_dir(&public_path).map_err(|e| StorageError::io(&public_path, e))?;

        Ok(())
    }

    /// Removes the on-disk directory for a database and all its contents.
    ///
    /// # Warning
    ///
    /// This is irreversible. The caller must ensure no active connections
    /// exist and that the WAL has been flushed before calling this.
    ///
    /// # Errors
    ///
    /// - [`StorageError::DirectoryNotFound`] if the database directory
    ///   does not exist.
    /// - [`StorageError::Io`] if the removal fails.
    pub fn drop_database_dir(&self, db_name: &str) -> Result<(), StorageError> {
        let db_path = self.database_path(db_name);

        if !db_path.exists() {
            return Err(StorageError::DirectoryNotFound(db_path));
        }

        std::fs::remove_dir_all(&db_path).map_err(|e| StorageError::io(&db_path, e))?;

        Ok(())
    }
}
