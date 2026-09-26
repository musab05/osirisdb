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
pub mod config;
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
pub use config::{AutoVacuumConfig, BgWriterConfig, CheckpointConfig, StorageConfig};
pub use error::StorageError;
pub use file::{FileRegistry, HeapFile, Storage};
pub use heap::TableHeap;
pub use log::{
    checkpoint_data::CheckpointData, checkpoint_manager::CheckpointManager,
    log_manager::LogManager, recovery::RecoveryEngine,
};
pub use pool::BufferPool;
pub use tuple::{RecordId, TUPLE_HEADER_SIZE, TupleHeader, TupleInfoMask};
pub use txn::{
    clog::{Clog, ClogStatus},
    snapshot::Snapshot,
    transaction::{Transaction, TxnStatus},
    transaction_manager::TransactionManager,
    visibility::is_tuple_visible,
};
