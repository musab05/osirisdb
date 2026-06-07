//! # rust_sql
//!
//! A modular, zero-dependency, hand-written SQL lexer and parser implemented in Rust.
//!
//! This crate parses raw SQL strings into a fully typed Abstract Syntax Tree (AST) representing
//! statements, expressions, constraints, data types, and operations.
//!
//! ## Modules
//!
//! - [`lexer`]: Tokenizes raw SQL input streams.
//! - [`ast`]: Defines typed Rust AST nodes for expressions and statements.
//! - [`parser`]: Parses token streams into AST nodes.
//!
//! ## Simple Example
//!
//! ```rust
//! use rust_sql::parser::Parser;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let sql = "SELECT id, name FROM users WHERE id = 42;";
//! let mut parser = Parser::new(sql);
//! let statements = parser.parse().map_err(|e| format!("{:?}", e))?;
//!
//! println!("{:#?}", statements);
//! # Ok(())
//! # }
//! ```

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod common;
pub mod catalog;