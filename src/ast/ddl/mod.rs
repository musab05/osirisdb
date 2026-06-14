//! AST structures representing Data Definition Language (DDL) statements.
//!
//! Supports schemas, tables, views, sequences, indexes, and constraints, along with drop and truncate statements.

pub mod alter;
pub mod column;
pub mod column_constraint;
pub mod table;
pub mod database;
pub mod drop;
pub mod drop_behaviour;
pub mod extension;
pub mod function;
pub mod generated_column;
pub mod index;
pub mod index_item;
pub mod partition;
pub mod procedure;
pub mod referential_action;
pub mod role;
pub mod schema;
pub mod sequence;
pub mod table_constraint;
pub mod tablespace;
pub mod trigger;
pub mod truncate;
pub mod type_statement;
pub mod view;

pub use alter::*;
pub use column::*;
pub use column_constraint::*;
pub use table::*;
pub use database::*;
pub use drop::*;
pub use drop_behaviour::*;
pub use extension::*;
pub use function::*;
pub use generated_column::*;
pub use index::*;
pub use index_item::*;
pub use partition::*;
pub use procedure::*;
pub use referential_action::*;
pub use role::*;
pub use schema::*;
pub use sequence::*;
pub use table_constraint::*;
pub use tablespace::*;
pub use trigger::*;
pub use truncate::*;
pub use type_statement::*;
pub use view::*;
