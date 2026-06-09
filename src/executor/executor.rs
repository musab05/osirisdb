use crate::{catalog::CatalogManager, common::symbol::Symbol};

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
}

impl Executor {
    /// Creates a new `Executor` for the given session.
    ///
    /// `catalog` is moved into the executor — the executor takes full
    /// ownership and is the only component that may write to it.
    /// `session_user` is the symbol of the currently connected role.
    pub fn new(catalog: CatalogManager, session_user: Symbol) -> Self {
        Self {
            catalog,
            session_user,
        }
    }
}
