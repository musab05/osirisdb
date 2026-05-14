use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    pub name: String,

    pub data_type: DataType,

    pub collation: Option<String>,

    pub constraints: Vec<ColumnConstraint>,

    pub generated: Option<GeneratedColumn>,
}

