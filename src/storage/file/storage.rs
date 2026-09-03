use crate::storage::{
    BufferPool, CheckpointManager, FileRegistry, LogManager, RecoveryEngine, error::StorageError,
    page::raw_page::PAGE_SIZE, pool::calculate_capacity,
};
use std::{
    fs::{remove_file, write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const SHUTDOWN_MARKER_FILE: &str = "clean_shutdown.marker";

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

    file_registry: Arc<FileRegistry>,
    buffer_pool: Arc<Mutex<BufferPool>>,
    log_manager: Option<Arc<LogManager>>,
    checkpoint_manager: Option<Arc<CheckpointManager>>,
}

impl Storage {
    /// Creates a new `Storage` instance rooted at `data_dir`.
    ///
    /// # Errors
    ///
    /// Returns [`StorageError::DirectoryNotFound`] if `data_dir`
    /// does not exist. The data directory must be created by the
    /// caller before initializing storage.
    fn build(
        data_dir: PathBuf,
        log_manager: Option<Arc<LogManager>>,
    ) -> Result<Self, StorageError> {
        let capacity = calculate_capacity(PAGE_SIZE, None);
        let file_registry =
            FileRegistry::open_or_create(&data_dir).map_err(|e| StorageError::io(&data_dir, e))?;
        Ok(Self {
            data_dir,
            file_registry: Arc::new(file_registry),
            buffer_pool: Arc::new(Mutex::new(BufferPool::new(capacity))),
            log_manager,
            checkpoint_manager: None,
        })
    }

    pub fn new(data_dir: impl Into<PathBuf>) -> Result<Self, StorageError> {
        let data_dir = data_dir.into();
        if !data_dir.exists() {
            return Err(StorageError::DirectoryNotFound(data_dir));
        }
        Self::build(data_dir, None)
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
        Self::build(data_dir, None)
    }

    /// Creates a Storage instance with an attached global LogManager for WAL durability.
    pub fn with_log_manager(
        data_dir: impl Into<PathBuf>,
        log_manager: Arc<LogManager>,
    ) -> Result<Self, StorageError> {
        let data_dir = data_dir.into();
        if !data_dir.exists() {
            return Err(StorageError::DirectoryNotFound(data_dir));
        }

        Self::build(data_dir, Some(log_manager))
    }

    pub fn with_checkpoint_manager(&mut self, checkpoint_manager: Arc<CheckpointManager>) {
        self.checkpoint_manager = Some(checkpoint_manager);
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

    /// Returns the path for logs
    pub fn log_path(&self, db: &str) -> Result<PathBuf, StorageError> {
        let db_path = self.database_path(db);
        let mut log_path = db_path;
        log_path.set_extension("log");
        Ok(log_path)
    }

    pub fn buffer_pool(&self) -> Arc<Mutex<BufferPool>> {
        Arc::clone(&self.buffer_pool)
    }

    pub fn file_registry(&self) -> Arc<FileRegistry> {
        Arc::clone(&self.file_registry)
    }

    pub fn log_manager(&self) -> Option<Arc<LogManager>> {
        self.log_manager.as_ref().map(Arc::clone)
    }

    pub fn shutdown(&mut self) -> Result<(), StorageError> {
        // Flushing all the dirty pages from buffer pool to disk
        {
            let mut bp = self.buffer_pool.lock().unwrap();
            bp.flush_all(self.log_manager.as_deref())?;
        }

        // Writing final checkpoint to WAL and checkpoint.meta
        if let Some(ref ckpt_mgr) = self.checkpoint_manager {
            ckpt_mgr.checkpoint()?;
        }

        // Flush WAL to stable storage
        if let Some(ref lm) = self.log_manager {
            lm.flush()?;
        }

        // Write clean-shutdown marker
        let marker_path = self.shutdown_marker_path();
        write(&marker_path, b"CLEAN_SHUTDOWN").map_err(|e| StorageError::io(&marker_path, e))?;

        Ok(())
    }

    pub fn shutdown_marker_path(&self) -> PathBuf {
        self.data_dir.join(SHUTDOWN_MARKER_FILE)
    }

    pub fn checkpoint_meta_path(&self) -> PathBuf {
        self.data_dir.join("checkpoint.meta")
    }

    pub fn has_clean_shutdown_marker(&self) -> bool {
        self.shutdown_marker_path().exists()
    }

    /// Checks for a clean-shutdown marker on startup:
    /// - If marker exists: removes marker and skips recovery (returns `Ok(false)`).
    /// - If marker does not exist: runs ARIES recovery (returns `Ok(true)`).
    pub fn recover_if_needed(
        &self,
        log_path: &Path,
        meta_path: &Path,
    ) -> Result<bool, StorageError> {
        let marker_path = self.shutdown_marker_path();

        if marker_path.exists() {
            // Clean shutdown delete marker so that future crashes are detected
            let _ = remove_file(&marker_path);
            return Ok(false);
        }

        // Unclean shutdown crash run ARIES recovery if WAL log exist
        if let Some(ref lm) = self.log_manager {
            if log_path.exists() {
                let recovery_engien = RecoveryEngine::new(
                    log_path,
                    meta_path,
                    Arc::clone(&self.file_registry),
                    Arc::clone(lm),
                );
                recovery_engien.recover()?;
            }
        }
        Ok(true)
    }
}
