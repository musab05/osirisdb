#[derive(Debug, Clone, PartialEq)]
pub struct ColumnDef {
    pub name: String,

    pub data_type: DataType,

    pub collation: Option<String>,

    pub constraints: Vec<ColumnCOnstraint>,

    pub generated: Option<GeneratedColumn>,
}
