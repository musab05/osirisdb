use crate::{catalog::CatalogManager, common::symbol::Symbol, storage::Storage};

/// The execution engine — receives bound statements and applies them
/// to the catalog and storage.
///
/// # Responsibilities
///
/// - Calls `CatalogManager` to register metadata for DDL statements
/// - Calls the storage layer to create or modify on-disk structures
/// - Returns execution results to the caller
///
/// # What the executor does NOT do
///
/// - Validate names — that is the binder's job
/// - Parse SQL — that is the parser's job
/// - Optimize queries — that is the planner's job
///
/// # Relationship to other layers
///
/// The executor owns `CatalogManager` — it is the only layer that
/// mutates the catalog. The binder only reads the catalog via a
/// shared reference.
pub struct Executor {
    /// The catalog manager — owns the in-memory catalog and the interner.
    ///
    /// The executor is the sole writer to the catalog.
    pub catalog: CatalogManager,

    /// The current session user symbol.
    ///
    /// Used as the default owner for objects created without an explicit
    /// OWNER clause. Resolved during session initialization.
    pub session_user: Symbol,
    /// The storage engine — manages on-disk layout.
    ///
    /// `None` when running in memory-only mode (e.g. tests that
    /// don't need disk I/O). `Some` in production.
    pub storage: Option<Storage>,
}

impl Executor {
    /// Creates a new `Executor` for the given session.
    ///
    /// `catalog` is moved into the executor — the executor takes full
    /// ownership and is the only component that may write to it.
    /// `session_user` is the symbol of the currently connected role.
    pub fn new(catalog: CatalogManager, session_user: Symbol, storage: Storage) -> Self {
        Self {
            catalog,
            session_user,
            storage: Some(storage),
        }
    }

    /// Creates an executor without storage — catalog only.
    ///
    /// Used in tests and early pipeline stages where disk I/O
    /// is not needed. Storage operations are silently skipped.
    pub fn new_in_memory(catalog: CatalogManager, session_user: Symbol) -> Self {
        Self {
            catalog,
            session_user,
            storage: None,
        }
    }
}
