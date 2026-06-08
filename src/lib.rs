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
//!

pub mod ast;
pub mod catalog;
pub mod common;
pub mod lexer;
pub mod parser;
pub mod binder;