use crate::ast::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Select(SelectStmt),
    // Insert(InsertStmt),
    // Update(UpdateStmt),
    // Delete(DeleteStmt),
    // Create(CreateStmt),
}