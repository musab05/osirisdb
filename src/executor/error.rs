use crate::{catalog::error::CatalogError, storage::StorageError};

/// Errors that can occur during statement execution.
///
/// Wraps errors from the catalog and storage layers so the caller
/// gets a single error type from the executor regardless of where
/// the failure originated.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionError {
    /// The catalog rejected the operation.
    Catalog(CatalogError),

    /// Storage failed to create or access the on-disk structure.
    /// Carries a human-readable description of the I/O failure.
    Storage(String),
}

impl From<CatalogError> for ExecutionError {
    fn from(e: CatalogError) -> Self {
        ExecutionError::Catalog(e)
    }
}

impl From<StorageError> for ExecutionError {
    fn from(e: StorageError) -> Self {
        ExecutionError::Storage(e.to_string())
    }
}

impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionError::Catalog(e) => write!(f, "catalog error: {}", e),
            ExecutionError::Storage(msg) => write!(f, "storage error: {}", msg),
        }
    }
}

impl std::error::Error for ExecutionError {}
