use crate::{ast::*, common::symbol::Symbol};

/// Represents the SQL `CreateTypeStmt` struct structure in the AST.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateTypeStmt {
    pub name: ObjectName,
    pub kind: TypeKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeKind {
    // CREATE TYPE mood AS ENUM ('happy', 'sad')
    Enum(Vec<Symbol>),

    // CREATE TYPE address AS (street TEXT, city TEXT)
    Composite(Vec<CompositeField>),

    // CREATE TYPE float_range AS RANGE (SUBTYPE = float8, ...)
    Range(RangeTypeDef),

    // CREATE DOMAIN positive_int AS INTEGER CHECK (VALUE > 0)
    Domain(DomainDef),

    // CREATE TYPE mytype AS BASE (INTERNALLENGTH = 4, ...)
    Base(BaseTypeDef),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompositeField {
    pub name: Symbol,
    pub data_type: DataType,
    pub collation: Option<Symbol>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RangeTypeDef {
    pub subtype: DataType,
    pub subtype_opclass: Option<Symbol>,
    pub collation: Option<Symbol>,
    pub canonical: Option<Symbol>,
    pub subtype_diff: Option<Symbol>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DomainDef {
    pub base_type: DataType,
    pub default: Option<Expr>,
    pub constraints: Vec<DomainConstraint>,
    pub not_null: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DomainConstraint {
    pub name: Option<Symbol>,
    pub check: Expr,
}

/// The base type is different from PG
#[derive(Debug, Clone, PartialEq)]
pub struct BaseTypeDef {
    pub internal_length: Option<i64>, // INTERNALLENGTH
    pub alignment: Option<Symbol>,    // ALIGNMENT = int4/double etc
    pub storage: Option<Symbol>,      // STORAGE = plain/extended etc
    pub passed_by_value: bool,        // PASSEDBYVALUE
    pub category: Option<char>,       // CATEGORY = 'N'
    pub preferred: bool,              // PREFERRED = true/false
    pub default: Option<Expr>,        // DEFAULT = value
    pub like_type: Option<DataType>,  // LIKE = INTEGER (our improvement)
    pub input_func: Option<Symbol>,   // INPUT = func (optional, unlike PG)
    pub output_func: Option<Symbol>,  // OUTPUT = func (optional, unlike PG)
}
