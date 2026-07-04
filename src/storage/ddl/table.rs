use std::fs::File;

use crate::storage::{Storage, StorageError};

impl Storage {
    /// Creates the on-disk file for a new table.
    ///
    /// # Directory layout created
    ///
    /// ```text
    /// data_dir/
    ///   {db_name}/
    ///     {schema_name}/
    ///       {table_name}   ← created here
    /// ```
    ///
    /// # Errors
    ///
    /// - [`StorageError::DirectoryNotFound`] if the parent schema directory does not exist.
    /// - [`StorageError::Io`] if the table file already exists, or if the filesystem
    ///   operation fails.
    pub fn create_table_file(
        &self,
        db: &str,
        schema: &str,
        table: &str,
    ) -> Result<(), StorageError> {
        let schema_path = self.schema_path(db, schema);

        if !schema_path.exists() {
            return Err(StorageError::DirectoryNotFound(schema_path));
        }

        let table_path = self.table_path(db, schema, table)?;
        if table_path.exists() {
            return Err(StorageError::Io {
                path: table_path.clone(),
                reason: "table file already exists".to_string(),
            });
        }

        File::create(&table_path).map_err(|e| StorageError::io(&table_path, e))?;
        Ok(())
    }
}
