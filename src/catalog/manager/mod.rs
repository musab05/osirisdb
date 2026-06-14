//! The `CatalogManager` struct and all mutation methods.
//!
//! Split into one file per object type — each file is
//! an `impl CatalogManager` block for that object's operations.

use crate::{
    catalog::catalog::Catalog,
    common::{interner::Interner, symbol::Symbol},
};

pub mod database;
pub mod schema;
pub mod table;
// pub mod role;
// pub mod tablespace;

/// The entry point for all catalog mutations.
///
/// Owns both the [`Catalog`] (data) and the [`Interner`] (name resolution).
/// The only component allowed to write to the catalog — all other layers
/// get a read-only reference.
///
/// Split across multiple impl blocks — one file per object type.
pub struct CatalogManager {
    /// The live catalog state.
    pub catalog: Catalog,

    /// Shared interner — names interned during parsing are resolved here.
    pub interner: Interner,
}

impl CatalogManager {
    /// Creates a new `CatalogManager` with an empty catalog.
    pub fn new(interner: Interner) -> Self {
        Self {
            catalog: Catalog::new(),
            interner,
        }
    }

    /// Returns `true` if a role with the given name exists.
    ///
    /// Stub — always returns `true` until role catalog is implemented.
    pub fn role_exists(&self, _name: Symbol) -> bool {
        true // TODO: implement when role catalog is added
    }

    /// Returns `true` if a tablespace with the given name exists.
    ///
    /// Stub — always returns `true` until tablespace catalog is implemented.
    pub fn tablespace_exists(&self, _name: Symbol) -> bool {
        true // TODO: implement when tablespace catalog is added
    }
}
