use std::{
    collections::{HashMap, hash_map::Entry},
    path::{Path, PathBuf},
    sync::RwLock,
};

/// Thread-safe bidirectional mapping between 32-bit `file_id` and physical disk `PathBuf`.
pub struct FileRegistry {
    id_to_path: RwLock<HashMap<u32, PathBuf>>,
    path_to_id: RwLock<HashMap<PathBuf, u32>>,
    next_file_id: RwLock<u32>,
}

impl FileRegistry {
    pub fn new() -> Self {
        Self {
            id_to_path: RwLock::new(HashMap::new()),
            path_to_id: RwLock::new(HashMap::new()),
            next_file_id: RwLock::new(1), // 0 is reserved for lifecycle records (BEGIN/COMMIT)
        }
    }

    /// Registers a physical file path and returns its unique `file_id`.
    /// If the path is already registered, returns the existing `file_id`.
    pub fn register(&self, path: impl AsRef<Path>) -> u32 {
        let path_buf = path.as_ref().to_path_buf();

        let mut path_to_id = self.path_to_id.write().unwrap();
        match path_to_id.entry(path_buf.clone()) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let mut next_id = self.next_file_id.write().unwrap();
                let file_id = *next_id;
                *next_id += 1;

                // Path -> ID
                entry.insert(file_id);

                // ID -> Path
                self.id_to_path.write().unwrap().insert(file_id, path_buf);

                file_id
            }
        }
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
