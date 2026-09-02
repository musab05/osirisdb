use std::{
    collections::HashMap,
    fs::{read, write},
    io::{Error, ErrorKind},
    path::{Path, PathBuf},
    str::from_utf8,
    sync::RwLock,
};

const MAGIC: &[u8; 4] = b"OSFR";
const VERSION: u32 = 1;

/// Thread-safe bidirectional mapping between 32-bit `file_id` and physical disk `PathBuf`.
pub struct FileRegistry {
    meta_path: PathBuf,
    data_dir: PathBuf,
    id_to_path: RwLock<HashMap<u32, PathBuf>>,
    path_to_id: RwLock<HashMap<PathBuf, u32>>,
    next_file_id: RwLock<u32>,
}

impl FileRegistry {
    fn load_from_disk(
        meta_path: &Path,
        data_dir: &Path,
    ) -> Result<(HashMap<u32, PathBuf>, HashMap<PathBuf, u32>, u32), Error> {
        let mut id_to_path: HashMap<u32, PathBuf> = HashMap::new();
        let mut path_to_id: HashMap<PathBuf, u32> = HashMap::new();

        if !meta_path.exists() {
            return Ok((id_to_path, path_to_id, 1));
        }

        let bytes = read(meta_path)?;
        if bytes.len() < 16 {
            return Ok((id_to_path, path_to_id, 1));
        }

        if &bytes[0..4] != MAGIC {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Invalid FileRegistry magic bytes",
            ));
        }

        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if version != VERSION {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("Unsupported FileRegistry version: {}", version),
            ));
        }

        let next_file_id = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let count = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;

        let mut cursor = 16;
        for _ in 0..count {
            if cursor + 8 > bytes.len() {
                break;
            }

            let file_id = u32::from_le_bytes(bytes[cursor..cursor + 4].try_into().unwrap());
            let path_len =
                u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
            cursor += 8;

            if cursor + path_len > bytes.len() {
                break;
            }
            let rel_str = from_utf8(&bytes[cursor..cursor + path_len])
                .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
            cursor += path_len;
            let full_path = data_dir.join(rel_str);
            id_to_path.insert(file_id, full_path.clone());
            path_to_id.insert(full_path, file_id);
        }
        Ok((id_to_path, path_to_id, next_file_id))
    }

    fn persist_to_disk(&self) -> Result<(), Error> {
        if self.meta_path.as_os_str().is_empty() {
            return Ok(());
        }

        let id_to_path = self.id_to_path.read().unwrap();
        let next_file_id = *self.next_file_id.read().unwrap();

        let mut buffer = Vec::new();
        buffer.extend_from_slice(MAGIC);
        buffer.extend_from_slice(&VERSION.to_le_bytes());
        buffer.extend_from_slice(&next_file_id.to_le_bytes());
        buffer.extend_from_slice(&(id_to_path.len() as u32).to_le_bytes());

        for (&file_id, path) in id_to_path.iter() {
            // Store relative path to stay relocatable
            let rel_path = path.strip_prefix(&self.data_dir).unwrap_or(path);
            let path_str = rel_path.to_string_lossy();
            let path_bytes = path_str.as_bytes();

            buffer.extend_from_slice(&file_id.to_le_bytes());
            buffer.extend_from_slice(&(path_bytes.len() as u32).to_le_bytes());
            buffer.extend_from_slice(path_bytes);
        }

        write(&self.meta_path, &buffer)?;
        Ok(())
    }

    pub fn open_or_create(data_dir: impl AsRef<Path>) -> Result<Self, Error> {
        let data_dir = data_dir.as_ref().to_path_buf();
        let meta_path = data_dir.join("file_registry.meta");

        let (id_to_path, path_to_id, next_file_id) = Self::load_from_disk(&meta_path, &data_dir)?;

        let registry = Self {
            meta_path,
            data_dir,
            id_to_path: RwLock::new(id_to_path),
            path_to_id: RwLock::new(path_to_id),
            next_file_id: RwLock::new(next_file_id),
        };

        // If file didn't exist, create it initially
        if !registry.meta_path.exists() {
            registry.persist_to_disk()?;
        }

        Ok(registry)
    }

    pub fn new() -> Self {
        Self {
            meta_path: PathBuf::new(),
            data_dir: PathBuf::new(),
            id_to_path: RwLock::new(HashMap::new()),
            path_to_id: RwLock::new(HashMap::new()),
            next_file_id: RwLock::new(1), // 0 is reserved for lifecycle records (BEGIN/COMMIT)
        }
    }

    /// Registers a physical file path and returns its unique `file_id`.
    /// If the path is already registered, returns the existing `file_id`.
    pub fn register(&self, path: impl AsRef<Path>) -> u32 {
        let path_buf = path.as_ref().to_path_buf();

        let maybe_id = {
            let path_to_id = self.path_to_id.read().unwrap();
            path_to_id.get(&path_buf).copied()
        };

        if let Some(id) = maybe_id {
            return id;
        }

        let mut path_to_id = self.path_to_id.write().unwrap();
        if let Some(&id) = path_to_id.get(&path_buf) {
            return id;
        }

        let file_id = {
            let mut next_id = self.next_file_id.write().unwrap();
            let id = *next_id;
            *next_id += 1;
            id
        };

        path_to_id.insert(path_buf.clone(), file_id);
        self.id_to_path.write().unwrap().insert(file_id, path_buf);

        // Drop lock before persisting to disk so persist_to_disk can acquire read lock
        drop(path_to_id);

        let _ = self.persist_to_disk();

        file_id
    }

    /// Returns the physical `PathBuf` corresponding to `file_id`, if registered.
    pub fn get_path(&self, file_id: u32) -> Option<PathBuf> {
        self.id_to_path.read().unwrap().get(&file_id).cloned()
    }

    /// Returns the `file_id` for a given path, if already registered.
    pub fn get_id(&self, path: impl AsRef<Path>) -> Option<u32> {
        self.path_to_id.read().unwrap().get(path.as_ref()).copied()
    }
}
