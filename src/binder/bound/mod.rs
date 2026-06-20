//! Bound AST nodes — fully resolved and validated statement representations.
//!
//! Each node in this module corresponds to a raw AST node from [`crate::ast`]
//! but with all names resolved, defaults filled in, and semantics validated.
//!
//! # Difference from raw AST nodes
//!
//! | Raw AST (`crate::ast`)         | Bound AST (`crate::binder::bound`)         |
//! |--------------------------------|--------------------------------------------|
//! | `owner: Option<Symbol>`        | `owner: Symbol` — always resolved          |
//! | `if_not_exists` checked later  | existence already validated by binder      |
//! | tablespace may not exist       | tablespace confirmed present in catalog    |
//! | connection limit unchecked     | connection limit validated >= -1           |
//!
//! # Adding new bound nodes
//!
//! When you add a new statement to the binder, create a corresponding
//! file here (e.g. `schema.rs`, `table.rs`) and add it to this module.

/// Bound `CREATE DATABASE` statement — resolved and validated,
/// ready for the executor to apply to the catalog.
pub mod database;
pub mod insert;
pub mod schema;
pub mod table;

pub use database::BoundCreateDatabaseStmt;
pub use insert::BoundInsertStmt;
pub use schema::BoundCreateSchemaStmt;
pub use table::BoundCreateTableStmt;
