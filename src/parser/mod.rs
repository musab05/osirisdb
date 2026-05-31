//! SQL parser implementation converting a stream of tokens into a fully validated AST.
//!
//! Organized using a trait-based extension pattern where statement categories are parsed
//! in modular submodules. Employs recursive descent for statements and Pratt parsing for operator expressions.

pub mod parser;
pub mod parser_error;
pub mod select;
pub mod binding_power;
pub mod expression;
pub mod statement;
pub mod create;
pub mod drop;
pub mod table;
pub mod modifiers;
pub mod truncate;
pub mod alter;
pub mod schema;
pub mod index;
pub mod view;
pub mod sequence;
pub mod type_statement;

pub use parser::Parser;
pub use parser_error::ParserError;