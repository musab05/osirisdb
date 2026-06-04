//! SQL parser implementation converting a stream of tokens into a fully validated AST.
//!
//! Organized using a trait-based extension pattern where statement categories are parsed
//! in modular submodules. Employs recursive descent for statements and Pratt parsing for operator expressions.

pub mod ddl;
pub mod expression;
pub mod parser;
pub mod parser_error;
pub mod query;
pub mod statement;

pub use parser::Parser;
pub use parser_error::ParserError;
