use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Select(SelectStmt),
    CreateTable(CreateStmt),
    Insert(InsertStmt),
    Update(UpdateStmt),
    Delete(DeleteStmt),
    DropTable(DropTableStmt),
    CreateIndex(CreateIndexStmt),
    Begin,
    Commit,
    Rollback,
}
