//! AST structures representing Data Manipulation Language (DML) statements.
//!
//! Covers SQL mutation operations such as insert, update, delete, upsert, and CTE boundaries.

pub mod assignment;
pub mod conflict_action;
pub mod conflict_target;
pub mod delete;
pub mod insert;
pub mod insert_source;
pub mod on_conflict;
pub mod update;

pub use assignment::*;
pub use conflict_action::*;
pub use conflict_target::*;
pub use delete::*;
pub use insert::*;
pub use insert_source::*;
pub use on_conflict::*;
pub use update::*;
