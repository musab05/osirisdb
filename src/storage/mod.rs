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

pub mod btree;
pub mod ddl;
pub mod error;
pub mod file;
pub mod heap;
pub mod page;
pub mod pool;
pub mod record_id;
pub mod tuple;

pub use btree::BPlusTreeIndex;
pub use error::StorageError;
pub use file::{HeapFile, Storage};
pub use heap::TableHeap;
pub use pool::BufferPool;
