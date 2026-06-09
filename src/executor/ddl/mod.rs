/// DDL execution — one file per object type.
///
/// Each file contains `impl Executor` methods for executing
/// the corresponding DDL statement variants.
pub mod database;
// pub mod schema;    ← add when ready
// pub mod table;     ← add when ready
