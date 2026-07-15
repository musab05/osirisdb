//! # OsirisDB
//!
//! A modular SQL database engine implemented in Rust, featuring a custom parser, binder, query planner, optimizer, catalog, and storage engine.
//!
//! > **Warning**: This project is currently under active development and is not yet ready for production use.
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
//!

pub mod ast;
pub mod binder;
pub mod catalog;
pub mod common;
pub mod executor;
pub mod lexer;
pub mod parser;
pub mod storage;

pub mod execution;
pub mod sql;
