#[derive(Debug, Clone, PartialEq)]
pub enum DataType {
    SmallInt,
    Int,
    BigInt,

    Boolean,

    Float,
    Double,

    Decimal(Option<u8>, Option<u8>),

    Char(Option<u64>),
    VarChar(Option<u64>),
    Text,

    Binary,
    VarBinary(Option<u64>),

    Json,
    JsonB,

    Date,
    Time,
    Timestamp,

    UUID,

    Array(Box<DataType>),

    Custom(Vec<String>),
}