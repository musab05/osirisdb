use crate::ast::*;

/// Represents a DDL statement to construct a sequence generator (`CREATE SEQUENCE`).
///
/// Sequences are database generators that emit sequential numeric values, commonly
/// used for auto-increment fields. Supports custom data types, increments, bounds,
/// caching, cycles, and column ownership specifications.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateSequenceStmt {
    /// The qualified name of the sequence.
    pub name: ObjectName,
    /// Skip error creation if the sequence already exists (`IF NOT EXISTS`).
    pub if_not_exists: bool,
    /// Optional sequence type (e.g. `AS INT`, `AS BIGINT`).
    pub data_type: Option<DataType>,
    /// Optional starting value of the sequence (`START WITH n`).
    pub start: Option<i64>,
    /// Optional increment step value (`INCREMENT BY n`).
    pub increment: Option<i64>,
    /// Optional minimum boundary value (`MINVALUE n`).
    pub minvalue: Option<i64>,
    /// Optional maximum boundary value (`MAXVALUE n`).
    pub maxvalue: Option<i64>,
    /// Number of pre-allocated sequence values cached in memory (`CACHE n`).
    pub cache: Option<i64>,
    /// If `Some(true)`, the sequence cycles when bounds are reached (`CYCLE`).
    pub cycle: Option<bool>,
    /// Optional table column that owns this sequence generator (`OWNED BY table.column`).
    pub owned_by: Option<Vec<String>>,
}
