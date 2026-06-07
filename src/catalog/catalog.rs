use crate::catalog::objects::database::DatabaseEntry;
use crate::common::symbol::Symbol;
use std::collections::HashMap;

/// The in-memory catalog — single source of truth for all database metadata.
///
/// Holds every object the engine knows about. All layers (binder, planner,
/// executor) read from here. Only [`CatalogManager`] writes to it.
pub struct Catalog {
    /// All databases keyed by their interned name symbol.
    pub databases: HashMap<Symbol, DatabaseEntry>,

    /// Monotonic counter for assigning unique object ids (OIDs).
    ///
    /// Incremented once per object created, never reused.
    next_oid: u32,
}

impl Catalog {
    /// Creates a new empty catalog with no databases.
    pub fn new() -> Self {
        Self {
            databases: HashMap::new(),
            next_oid: 1, // 0 is reserved as a null/invalid OID
        }
    }

    /// Allocates and returns the next available OID.
    pub(crate) fn next_oid(&mut self) -> u32 {
        let oid = self.next_oid;
        self.next_oid += 1;
        oid
    }
}

impl Default for Catalog {
    fn default() -> Self {
        Self::new()
    }
}
