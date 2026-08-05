use crate::storage::error::StorageError;
use std::path::{Path, PathBuf};

/// The storage engine — manages the on-disk layout for all database objects.
///
/// # On-disk layout
///
/// ```text
/// data_dir/
///   mydb/                  ← one directory per database
///     public/              ← default schema directory (created with database)
///   otherdb/
///     public/
/// ```
///
/// Every database gets its own directory under `data_dir`.
/// Every schema gets a subdirectory under its database directory.
/// Tables, indexes, and sequences will each get their own files
/// under their schema directory.
///
/// # Zero dependencies
///
/// Uses only `std::fs` — no async runtime, no external crates.
/// All operations are synchronous. Async I/O can be layered on
/// top later without changing this interface.
pub struct Storage {
    /// Root directory where all database data is stored.
    ///
    /// Every database directory lives directly under this path.
    /// Must exist before `Storage::new` is called.
    data_dir: PathBuf,
}

impl Storage {
    /// Creates a new `Storage` instance rooted at `data_dir`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::DirectoryNotFound`] if `data_dir`
    /// does not exist. The data directory must be created by the
    /// caller before initializing storage.
    pub fn new(data_dir: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let data_dir = data_dir.into();
        if !data_dir.exists() {
            return Err(StorageError::DirectoryNotFound(data_dir));
        }
        Ok(Self { data_dir })
    }

    /// Creates a `Storage` instance and creates `data_dir` if it
    /// does not already exist.
    ///
    /// Useful for first-time initialization and tests.
    pub fn new_or_create(data_dir: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let data_dir = data_dir.into();
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir).map_err(|e| StorageError::io(&data_dir, e))?;
        }
        Ok(Self { data_dir })
    }

    /// Returns the root data directory path.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Returns the expected on-disk path for a database directory.
    ///
    /// Does not check whether the directory exists.
    pub fn database_path(&self, db_name: &str) -> PathBuf {
        self.data_dir.join(db_name)
    }

    /// Returns `true` if the on-disk directory for `db_name` exists.
    pub fn database_dir_exists(&self, db_name: &str) -> bool {
        self.database_path(db_name).exists()
    }

    /// Returns the expected on-disk path for a schema directory.
    ///
    /// Does not check whether the directory exists.
    pub fn schema_path(&self, db_name: &str, schema_name: &str) -> PathBuf {
        self.database_path(db_name).join(schema_name)
    }

    /// Returns `true` if the on-disk directory for `schema_name` exists
    /// within `db_name`.
    pub fn schema_dir_exists(&self, db_name: &str, schema_name: &str) -> bool {
        self.schema_path(db_name, schema_name).exists()
    }

    // storage.rs additions
    pub fn table_path(&self, db: &str, schema: &str, table: &str) -> Result<PathBuf, StorageError> {
        if !self.database_dir_exists(db) {
            return Err(StorageError::DirectoryNotFound(self.database_path(db)));
        }
        if !self.schema_dir_exists(db, schema) {
            return Err(StorageError::DirectoryNotFound(
                self.schema_path(db, schema),
            ));
        }
        Ok(self.schema_path(db, schema).join(format!("{}.dat", table)))
    }

    /// Returns the expected on-disk path for a table's overflow (TOAST) file.
    pub fn toast_path(&self, db: &str, schema: &str, table: &str) -> Result<PathBuf, StorageError> {
        let table_dat_path = self.table_path(db, schema, table)?;
        let mut toast_path = table_dat_path;
        toast_path.set_extension("toast");
        Ok(toast_path)
    }
}
