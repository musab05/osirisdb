pub enum Statement {
    Select(SelectStmt),
    CreateTable(CreateTableStmt),
    Insert(InsertStmt),
}
