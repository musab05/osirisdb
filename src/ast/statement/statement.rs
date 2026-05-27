use crate::ast::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    Select(SelectStmt),
    CreateTable(CreateStmt),
    AlterTable(AlterTableStmt),
    DropTable(DropTableStmt),
    TruncateTable(TruncateStmt),
    CreateSchema(CreateSchemaStmt),
    CreateSequence(CreateSequenceStmt),
    Insert(InsertStmt),
    Update(UpdateStmt),
    Delete(DeleteStmt),
    CreateIndex(CreateIndexStmt),
    CreateView(CreateViewStmt),
    AlterView(AlterViewStmt),
    DropView(DropViewStmt),
    Begin,
    Commit,
    Rollback,
}
