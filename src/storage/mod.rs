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

pub mod buffer_pool;
pub mod ddl;
pub mod error;
pub mod heap_file;
pub mod index_page;
pub mod page;
pub mod storage;
pub mod table_heap;
pub mod tuple;
pub mod b_plus_tree_index;
pub mod record_id;

pub use buffer_pool::BufferPool;
pub use error::StorageError;
pub use heap_file::HeapFile;
pub use storage::Storage;
pub use table_heap::TableHeap;
