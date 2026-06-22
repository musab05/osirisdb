//! The binder — semantic analysis and name resolution layer.
//!
//! Sits between the parser and the executor in the pipeline:
//!
//! ```text
//! SQL → Parser → AST → Binder → Bound AST → Executor → Catalog
//! ```
//!
//! # Responsibilities
//!
//! - **Name resolution** — confirms every referenced object exists in the catalog
//! - **Default resolution** — fills in omitted values (e.g. `OWNER` → session user)
//! - **Semantic validation** — catches errors the parser cannot, such as unknown
//!   roles, invalid connection limits, or referencing a table in the wrong schema
//!
//! # What the binder does NOT do
//!
//! - Modify the catalog — that is the executor's job
//! - Parse SQL — that is the parser's job
//! - Optimize queries — that is the planner's job
//!
//! # Structure
//!
//! - [`binder`] — the [`Binder`] struct and all `bind_*` methods
//! - [`error`] — [`BindError`] variants for every validation failure
//! - [`bound`] — fully resolved AST nodes produced by the binder,
//!   one per statement type (e.g. [`BoundCreateDatabaseStmt`])

/// The [`Binder`] struct and all `bind_*` methods, one per statement type.
pub mod binder;

/// [`BindError`] — all errors the binder can produce during validation.
pub mod error;

/// Bound AST nodes — fully resolved and validated statements
/// ready to be handed directly to the executor.
pub mod bound;

pub mod ddl;
pub mod dml;
pub mod query;

pub use binder::Binder;
pub use error::BindError;
