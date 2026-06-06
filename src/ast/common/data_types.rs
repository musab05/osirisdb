use crate::common::symbol::Symbol;

/// Represents the SQL data types supported by the SQL parser.
///
/// This enum maps standard SQL and PostgreSQL-specific types, including numeric,
/// character, binary, temporal, JSON, UUID, and nested array types.
#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    /// 16-bit signed integer (PostgreSQL `SMALLINT` or `INT2`).
    SmallInt,
    /// 32-bit signed integer (PostgreSQL `INTEGER`, `INT`, or `INT4`).
    Int,
    /// 64-bit signed integer (PostgreSQL `BIGINT` or `INT8`).
    BigInt,

    /// Boolean logical type representing `TRUE`, `FALSE`, or `NULL`.
    Boolean,

    /// Single-precision floating-point number (PostgreSQL `REAL` or `FLOAT4`).
    Float,
    /// Double-precision floating-point number (PostgreSQL `DOUBLE PRECISION` or `FLOAT8`).
    Double,

    /// Exact numeric type with custom precision and scale (e.g. `DECIMAL(10, 2)`).
    /// Holds `(Option<precision>, Option<scale>)`.
    Decimal(Option<u8>, Option<u8>),

    /// Fixed-length character string (e.g. `CHAR(10)`).
    Char(Option<u64>),
    /// Variable-length character string with optional max length (e.g. `VARCHAR(255)`).
    VarChar(Option<u64>),
    /// Variable-length character string with unlimited length.
    Text,

    /// Fixed-length raw binary string.
    Binary,
    /// Variable-length raw binary string with optional max length.
    VarBinary(Option<u64>),

    /// Standard text-based JSON representation.
    Json,
    /// PostgreSQL binary-stored JSONB representation.
    JsonB,

    /// Calendar date (year, month, day).
    Date,
    /// Time of day without time zone.
    Time,
    /// Date and time with or without time zone support.
    Timestamp,

    /// Universally Unique Identifier (128-bit UUID).
    UUID,

    /// Multi-dimensional arrays of a specific base type (e.g., `INT[]`).
    Array(Box<DataType>),

    /// Custom, user-defined or schema-qualified types (e.g., `public.mood`, `my_type`).
    /// Keeps track of a list of identifiers representing the qualified path.
    Custom(Vec<Symbol>),
}
