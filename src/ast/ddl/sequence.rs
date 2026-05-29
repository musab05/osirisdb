use crate::ast::*;

#[derive(Debug, Clone, PartialEq)]
pub struct CreateSequenceStmt {
    pub name: ObjectName,
    pub if_not_exists: bool,
    pub data_type: Option<DataType>,   // AS integer/bigint/smallint
    pub start: Option<i64>,            // START WITH n
    pub increment: Option<i64>,        // INCREMENT BY n
    pub minvalue: Option<i64>,         // MINVALUE n
    pub maxvalue: Option<i64>,         // MAXVALUE n
    pub cache: Option<i64>,            // CACHE n
    pub cycle: Option<bool>,           // true = CYCLE, false = NO CYCLE
    pub owned_by: Option<Vec<String>>, // OWNED BY table.col, None = NONE
}
