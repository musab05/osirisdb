//! The storage engine — manages the on-disk layout for all database objects.
//!
//! Sits at the bottom of the pipeline:
//!
//! ```text
//! Executor → Storage → Disk
//! ```
//!
//! # On-disk layout
//!
//! ```text
//! data_dir/
//!   {database}/
//!     {schema}/
//!       {table}.dat     ← heap file (rows)
//!       {index}.idx     ← index file
//! ```
//!
//! # Structure
//!
//! - [`storage`] — the [`Storage`] struct and path helpers
//! - [`error`]   — [`StorageError`] variants
//! - [`ddl`]     — DDL storage operations, one file per object type

pub mod ddl;
pub mod error;
pub mod heap_file;
pub mod page;
pub mod storage;

pub use error::StorageError;
pub use storage::Storage;
