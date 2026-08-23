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
pub mod log;
pub mod page;
pub mod pool;
pub mod toast;
pub mod tuple;
pub mod txn;
pub mod util;

pub use btree::BPlusTreeIndex;
pub use error::StorageError;
pub use file::{FileRegistry, HeapFile, Storage};
pub use heap::TableHeap;
pub use log::log_manager::LogManager;
pub use pool::BufferPool;
pub use tuple::record_id::RecordId;
pub use txn::{
    transaction::{Transaction, TxnStatus},
    transaction_manager::TransactionManager,
};
