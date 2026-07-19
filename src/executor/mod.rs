//! The execution engine — applies bound statements to the catalog and storage.
//!
//! Sits at the bottom of the pipeline just above storage:
//!
//! ```text
//! BoundStmt → Executor → CatalogManager → Catalog
//!                      → Storage        → Disk
//! ```
//!
//! # Structure
//!
//! - [`executor`] — the [`Executor`] struct
//! - [`error`]    — [`ExecutionError`] variants
//! - [`result`]   — [`ExecutionResult`] variants — what the executor returns
//! - [`ddl`]      — DDL execution, one file per object type

pub mod ddl;
pub mod dml;
pub mod error;
pub mod executor;
pub mod query;
pub mod result;
pub mod session;

pub use error::ExecutionError;
pub use executor::Executor;
pub use result::ExecutionResult;
