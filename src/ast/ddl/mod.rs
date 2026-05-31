//! AST structures representing Data Definition Language (DDL) statements.
//!
//! Supports schemas, tables, views, sequences, indexes, and constraints, along with drop and truncate statements.

pub mod alter;
pub mod column_constraint;
pub mod column;
pub mod create;
pub mod drop_behaviour;
pub mod drop;
pub mod generated_column;
pub mod index_item;
pub mod index;
pub mod partition;
pub mod referential_action;
pub mod schema;
pub mod sequence;
pub mod table_constraint;
pub mod truncate;
pub mod view;
pub mod type_statement;

pub use alter::*;
pub use column_constraint::*;
pub use column::*;
pub use create::*;
pub use drop_behaviour::*;
pub use drop::*;
pub use generated_column::*;
pub use index_item::*;
pub use index::*;
pub use partition::*;
pub use referential_action::*;
pub use schema::*;
pub use sequence::*;
pub use table_constraint::*;
pub use truncate::*;
pub use view::*;
pub use type_statement::*;