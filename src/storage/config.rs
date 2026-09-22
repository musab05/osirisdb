use crate::storage::{error::StorageError, page::raw_page::PAGE_SIZE};
use std::{fs, path::Path, time::Duration};

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
        if self.shared_buffers == 0 {
            crate::storage::pool::calculate_capacity(PAGE_SIZE, None)
        } else {
            (self.shared_buffers / PAGE_SIZE).max(1)
        }
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
        let mut config = Self::default();
        let kvs = parse_key_value_lines(content, path_context)?;

        for (key, val) in kvs {
            match key.as_str() {
                "shared_buffers" | "shared_buffer" => {
                    config.shared_buffers = parse_bytes(&val, &key, path_context)?;
                }
                "wal_buffers" => config.wal_buffers = parse_bytes(&val, &key, path_context)?,
                "checkpoint_timeout" => {
                    config.checkpoint.timeout = parse_duration(&val, &key, path_context)?;
                }
                "checkpoint_completion_target" => {
                    let target: f64 = val.parse().map_err(|_| {
                        StorageError::CorruptedData(format!(
                            "Invalid float for '{}' in {}: {}",
                            key,
                            path_context.display(),
                            val
                        ))
                    })?;
                    if !(0.0..=1.0).contains(&target) {
                        return Err(StorageError::CorruptedData(format!(
                            "checkpoint_completion_target must be between 0.0 and 1.0, got {}",
                            target
                        )));
                    }
                    config.checkpoint.completion_target = target;
                }
                "bgwriter_delay" => {
                    config.bgwriter.delay = parse_duration(&val, &key, path_context)?;
                }
                "bgwriter_lru_maxpages" => {
                    config.bgwriter.lru_maxpages = parse_usize(&val, &key, path_context)?;
                }
                "autovacuum" => config.autovacuum.enabled = parse_bool(&val, &key, path_context)?,
                "autovacuum_naptime" => {
                    config.autovacuum.naptime = parse_duration(&val, &key, path_context)?;
                }
                "autovacuum_vacuum_threshold" => {
                    config.autovacuum.vacuum_threshold = parse_usize(&val, &key, path_context)?;
                }
                "autovacuum_vacuum_scale_factor" => {
                    config.autovacuum.vacuum_scale_factor = val.parse().map_err(|_| {
                        StorageError::CorruptedData(format!(
                            "Invalid float for '{}' in {}: {}",
                            key,
                            path_context.display(),
                            val
                        ))
                    })?;
                }
                "autovacuum_cost_limit" => {
                    config.autovacuum.cost_limit = parse_usize(&val, &key, path_context)?;
                }
                _ => {
                    // Ignore unknown parameters or log warning (Postgres forwards or ignores)
                }
            }
        }

        Ok(config)
    }
}

fn parse_key_value_lines(
    content: &str,
    path: &Path,
) -> Result<Vec<(String, String)>, StorageError> {
    let mut pairs = Vec::new();

    for (line_num, raw_line) in content.lines().enumerate() {
        let line = raw_line.trim();
        // Skip blank lines and full-line comments
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        // Strip inline comments if present
        let clean_line = match line.find(['#', ';']) {
            Some(idx) => &line[..idx],
            None => line,
        };
        let clean_line = clean_line.trim();
        if clean_line.is_empty() {
            continue;
        }

        if let Some((k, v)) = clean_line.split_once('=') {
            let key = k.trim().to_lowercase();
            let mut val = v.trim();
            // Strip enclosing quotes if present (e.g. '128MB' or "128MB")
            if (val.starts_with('\'') && val.ends_with('\''))
                || (val.starts_with('"') && val.ends_with('"'))
            {
                if val.len() >= 2 {
                    val = &val[1..val.len() - 1];
                }
            }
            pairs.push((key, val.trim().to_string()));
        } else {
            return Err(StorageError::CorruptedData(format!(
                "Syntax error in {}:{}: Expected key = value, found '{}'",
                path.display(),
                line_num + 1,
                line
            )));
        }
    }

    Ok(pairs)
}

/// Parses memory strings with units like 128MB, 64kB, 1GB, 4096.
fn parse_bytes(val: &str, key: &str, path: &Path) -> Result<usize, StorageError> {
    let s = val.trim();
    let split_pos = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let (num_str, unit_str) = s.split_at(split_pos);

    let num: usize = num_str.parse().map_err(|_| {
        StorageError::CorruptedData(format!(
            "Invalid numeric value '{}' for key '{}' in {}",
            val,
            key,
            path.display()
        ))
    })?;

    let unit = unit_str.trim().to_lowercase();
    let multiplier = match unit.as_str() {
        "" | "b" => 1,
        "kb" | "k" => 1024,
        "mb" | "m" => 1024 * 1024,
        "gb" | "g" => 1024 * 1024 * 1024,
        _ => {
            return Err(StorageError::CorruptedData(format!(
                "Unknown byte unit '{}' in '{}' for key '{}'",
                unit_str, val, key
            )));
        }
    };

    Ok(num.saturating_mul(multiplier))
}

/// Parses duration strings like 300s, 5min, 200ms, 1h.
fn parse_duration(val: &str, key: &str, path: &Path) -> Result<Duration, StorageError> {
    let s = val.trim();
    let split_pos = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let (num_str, unit_str) = s.split_at(split_pos);

    let num: u64 = num_str.parse().map_err(|_| {
        StorageError::CorruptedData(format!(
            "Invalid duration number '{}' for key '{}' in {}",
            val,
            key,
            path.display()
        ))
    })?;

    let unit = unit_str.trim().to_lowercase();
    match unit.as_str() {
        "ms" => Ok(Duration::from_millis(num)),
        "s" | "" => Ok(Duration::from_secs(num)),
        "min" | "m" => Ok(Duration::from_secs(num * 60)),
        "h" => Ok(Duration::from_secs(num * 3600)),
        _ => Err(StorageError::CorruptedData(format!(
            "Unknown time unit '{}' in '{}' for key '{}'",
            unit_str, val, key
        ))),
    }
}

/// Parses boolean values: on/off, true/false, yes/no, 1/0.
fn parse_bool(val: &str, key: &str, path: &Path) -> Result<bool, StorageError> {
    match val.trim().to_lowercase().as_str() {
        "true" | "on" | "yes" | "1" => Ok(true),
        "false" | "off" | "no" | "0" => Ok(false),
        _ => Err(StorageError::CorruptedData(format!(
            "Invalid boolean '{}' for key '{}' in {}",
            val,
            key,
            path.display()
        ))),
    }
}

fn parse_usize(val: &str, key: &str, path: &Path) -> Result<usize, StorageError> {
    val.trim().parse::<usize>().map_err(|_| {
        StorageError::CorruptedData(format!(
            "Invalid integer '{}' for key '{}' in {}",
            val,
            key,
            path.display()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let conf = StorageConfig::default();
        assert_eq!(conf.shared_buffers, 128 * 1024 * 1024);
        assert_eq!(conf.wal_buffers, 16 * 1024 * 1024);
        assert_eq!(conf.checkpoint.timeout.as_secs(), 300);
        assert_eq!(conf.checkpoint.completion_target, 0.9);
        assert_eq!(conf.bgwriter.delay.as_millis(), 200);
        assert_eq!(conf.bgwriter.lru_maxpages, 100);
        assert!(conf.autovacuum.enabled);
        assert_eq!(conf.autovacuum.naptime.as_secs(), 60);
        assert_eq!(conf.autovacuum.vacuum_threshold, 50);
        assert_eq!(conf.autovacuum.vacuum_scale_factor, 0.2);
        assert_eq!(conf.autovacuum.cost_limit, 200);
    }

    #[test]
    fn test_config_parsing() {
        let conf_str = r#"
            # OsirisDB configuration
            shared_buffers = 256MB
            wal_buffers = 32MB
            checkpoint_timeout = 10min
            checkpoint_completion_target = 0.75
            bgwriter_delay = 50ms
            bgwriter_lru_maxpages = 250
            autovacuum = off
            autovacuum_naptime = 30s
            autovacuum_vacuum_threshold = 100
            autovacuum_vacuum_scale_factor = 0.15
            autovacuum_cost_limit = 500
        "#;

        let conf = StorageConfig::parse(conf_str, Path::new("osirisdb.conf")).unwrap();
        assert_eq!(conf.shared_buffers, 256 * 1024 * 1024);
        assert_eq!(conf.wal_buffers, 32 * 1024 * 1024);
        assert_eq!(conf.checkpoint.timeout, Duration::from_secs(600));
        assert_eq!(conf.checkpoint.completion_target, 0.75);
        assert_eq!(conf.bgwriter.delay, Duration::from_millis(50));
        assert_eq!(conf.bgwriter.lru_maxpages, 250);
        assert!(!conf.autovacuum.enabled);
        assert_eq!(conf.autovacuum.naptime, Duration::from_secs(30));
        assert_eq!(conf.autovacuum.vacuum_threshold, 100);
        assert_eq!(conf.autovacuum.vacuum_scale_factor, 0.15);
        assert_eq!(conf.autovacuum.cost_limit, 500);
    }

    #[test]
    fn test_config_units_and_quotes() {
        let conf_str = r#"
            shared_buffers = '64kB'
            wal_buffers = "1GB"
            checkpoint_timeout = 2h
            checkpoint_completion_target = 0.5
            autovacuum = "yes"
        "#;

        let conf = StorageConfig::parse(conf_str, Path::new("osirisdb.conf")).unwrap();
        assert_eq!(conf.shared_buffers, 64 * 1024);
        assert_eq!(conf.wal_buffers, 1024 * 1024 * 1024);
        assert_eq!(conf.checkpoint.timeout, Duration::from_secs(7200));
        assert_eq!(conf.checkpoint.completion_target, 0.5);
        assert!(conf.autovacuum.enabled);
    }

    #[test]
    fn test_config_invalid_completion_target() {
        let conf_str = "checkpoint_completion_target = 1.5";
        assert!(StorageConfig::parse(conf_str, Path::new("test.conf")).is_err());

        let conf_str2 = "checkpoint_completion_target = -0.1";
        assert!(StorageConfig::parse(conf_str2, Path::new("test.conf")).is_err());
    }

    #[test]
    fn test_buffer_pool_frames() {
        let mut conf = StorageConfig::default();
        conf.shared_buffers = 8192 * 100; // 100 pages
        assert_eq!(conf.buffer_pool_frames(), 100);

        // 0 should trigger fallback to dynamic detection
        conf.shared_buffers = 0;
        assert!(conf.buffer_pool_frames() >= 64);
    }
}
