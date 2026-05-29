use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub struct InsertStmt {
    pub table: ObjectName,
    pub columns: Vec<String>,
    pub source: InsertSource,
    pub on_conflict: Option<OnConflict>,
    pub returning: Vec<SelectItem>,
}

