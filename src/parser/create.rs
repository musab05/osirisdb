#[derive(Debug, Clone, PartialEq)]
pub struct CreateTableStmt {
    pub if_not_exist: bool,
    pub temporary: bool,
    pub unlogged: bool,

    pub name: Vec<String>,

    pub columns: Vec<ColumnDef>,
    pub constraints: Vec<TableConstraint>,

    pub inherits: Vec<Vec<String>>,
    pub partitions: Vec<PartionsClause>,

    pub with_options: Vec<SqlOption>,

    pub table_space: Option<String>,

    pub on_commit: Option<OnCommit>,

    pub as_query: Option<SelectStmt>,
}
