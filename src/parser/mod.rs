//! SQL parser module implementing a recursive descent parser with Pratt parsing for expressions.
//!
//! This module transforms a stream of [`Token`](crate::lexer::token::Token)s (produced by the lexer) into an abstract syntax
//! tree (AST) of SQL statements. The parser is organized using a **trait-based extension pattern**:
//! the core [`Parser`] struct lives in [`parser`], and each SQL statement category (SELECT, CREATE,
//! ALTER, DROP, etc.) is implemented as an `impl Parser` block in its own submodule. This keeps
//! the parsing logic modular and easy to extend.
//!
//! ## Architecture
//!
//! - **Recursive descent** is used for statement-level grammar (CREATE TABLE, SELECT, ALTER, etc.).
//! - **Pratt parsing** (top-down operator precedence) is used for expression parsing via
//!   [`binding_power`], allowing correct handling of operator precedence and associativity.
//! - The parser uses a two-token lookahead (`current` + `peek`) to make parsing decisions without
//!   backtracking.
//!
//! ## Submodule Layout
//!
//! | Module            | Responsibility                                |
//! |-------------------|-----------------------------------------------|
//! | [`parser`]        | Core `Parser` struct and token navigation      |
//! | [`parser_error`]  | Error type with span information               |
//! | [`modifiers`]     | `CreateModifiers` for CREATE statement flags   |
//! | [`binding_power`] | Pratt precedence tables for operators          |
//! | [`statement`]     | Top-level statement dispatch                   |
//! | [`expression`]    | Expression parsing (literals, operators, etc.) |
//! | [`select`]        | SELECT statements, CTEs, JOINs, set ops        |
//! | [`create`]        | CREATE dispatcher (TABLE, VIEW, INDEX, etc.)   |
//! | [`mod@drop`]      | DROP statements                                |
//! | [`alter`]         | ALTER statements                               |
//! | [`table`]         | CREATE/ALTER/DROP TABLE details                 |
//! | [`truncate`]      | TRUNCATE TABLE                                 |
//! | [`schema`]        | CREATE SCHEMA                                  |
//! | [`index`]         | CREATE INDEX                                   |
//! | [`view`]          | CREATE VIEW                                    |
//! | [`sequence`]      | CREATE SEQUENCE                                |

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

pub use parser::Parser;
pub use parser_error::ParserError;