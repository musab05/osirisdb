use crate::ast::*;

/// Represents a DDL statement to construct a database search index (`CREATE INDEX`).
///
/// Supports optional features like unique indexes, specific index methods (e.g. btree, hash),
/// columns inclusion, sorting preferences, and partial index filter criteria (`WHERE`).
#[derive(Debug, Clone, PartialEq)]
pub struct CreateIndexStmt {
    /// Enforces uniqueness of key values in the index (`UNIQUE`).
    pub unique: bool,
    /// Skip error creation if the index already exists (`IF NOT EXISTS`).
    pub if_not_exist: bool,
    /// Optional index identifier name.
    pub name: Option<String>,
    /// The target table to index.
    pub table: ObjectName,
    /// Optional index method name (e.g., `btree`, `hash`, `gist`).
    pub method: Option<String>,
    /// The columns or expressions that make up the index keys.
    pub columns: Vec<IndexItem>,
    /// Optional payload columns included in the index but not part of keys (`INCLUDE`).
    pub include: Vec<String>,
    /// Optional filter predicate for partial index (`WHERE`).
    pub where_: Option<Expr>,
}
