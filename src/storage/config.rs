use crate::storage::{error::StorageError, page::raw_page::PAGE_SIZE};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, PartialEq)]
pub struct StorageConfig {
    /// Buffer pool memory size in bytes (shared_buffers).
    /// Default: 128 MB (or dynamic calculation).
    pub shared_buffers: usize,

    /// WAL active/flush buffer size in bytes (wal_buffers).
    /// Default: 16 MB.
    pub wal_buffers: usize,

    /// Checkpoint parameters.
    pub checkpoint: CheckpointConfig,

    /// Background writer parameters.
    pub bgwriter: BgWriterConfig,

    /// Autovacuum parameters.
    pub autovacuum: AutoVacuumConfig,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CheckpointConfig {
    /// Maximum elapsed time between automatic checkpoints.
    pub timeout: Duration,
    /// Target completion percentage (0.0 to 1.0) to spread I/O load.
    pub completion_target: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BgWriterConfig {
    /// Sleep duration between bgwriter rounds.
    pub delay: Duration,
    /// Maximum number of LRU pages flushed per round.
    pub lru_maxpages: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AutoVacuumConfig {
    /// Whether background autovacuum daemon is active.
    pub enabled: bool,
    /// Sleep time between daemon sweeps.
    pub naptime: Duration,
    /// Minimum dead tuples required to trigger vacuum.
    pub vacuum_threshold: usize,
    /// Fraction of table size added to threshold.
    pub vacuum_scale_factor: f64,
    /// Cost limit per cycle to throttle disk I/O.
    pub cost_limit: usize,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            shared_buffers: 128 * 1024 * 1024, // 128 MB
            wal_buffers: 16 * 1024 * 1024,     // 16 MB
            checkpoint: CheckpointConfig::default(),
            bgwriter: BgWriterConfig::default(),
            autovacuum: AutoVacuumConfig::default(),
        }
    }
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(300), // 5 min
            completion_target: 0.9,
        }
    }
}

impl Default for BgWriterConfig {
    fn default() -> Self {
        Self {
            delay: Duration::from_millis(200),
            lru_maxpages: 100,
        }
    }
}

impl Default for AutoVacuumConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            naptime: Duration::from_secs(60),
            vacuum_threshold: 50,
            vacuum_scale_factor: 0.2,
            cost_limit: 200,
        }
    }
}

impl StorageConfig {
    /// Returns the number of frames allocated for the buffer pool.
    pub fn buffer_pool_frames(&self) -> usize {
        (self.shared_buffers / PAGE_SIZE).max(1)
    }
}

impl StorageConfig {
    /// Loads configuration from an `osirisdb.conf` file.
    /// If the file does not exist, returns the default configuration.
    pub fn load_or_default(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Self::default());
        }
        Self::from_file(path)
    }

    /// Reads and parses an `osirisdb.conf` file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref();
        let contents = fs::read_to_string(path).map_err(|e| StorageError::io(path, e))?;
        Self::parse(&contents, path)
    }

    /// Parses the raw string contents of an `osirisdb.conf` file.
    pub fn parse(content: &str, path_context: &Path) -> Result<Self, StorageError> {
        todo!()
    }
}
