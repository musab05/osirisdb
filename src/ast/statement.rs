use crate::ast::*;

/// Represents the top-level SQL statements supported by the parser.
///
/// This is the root node of the parsed SQL syntax tree (AST). A query string
/// can contain one or more statements separated by semicolons.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// A `SELECT` query statement representing data retrieval, CTEs, joins, aggregates, and set ops.
    Select(SelectStmt),
    /// A `CREATE TABLE` DDL statement representing table schemas, column lists, and table constraints.
    CreateTable(CreateStmt),
    /// An `ALTER TABLE` DDL statement representing column additions/removals, default modifications, etc.
    AlterTable(AlterTableStmt),
    /// A `DROP TABLE` DDL statement.
    DropTable(DropTableStmt),
    /// A `TRUNCATE TABLE` DDL statement.
    TruncateTable(TruncateStmt),
    /// A `CREATE SCHEMA` DDL statement.
    CreateSchema(CreateSchemaStmt),
    /// A `CREATE SEQUENCE` DDL statement.
    CreateSequence(CreateSequenceStmt),
    /// An `INSERT` DML statement.
    Insert(InsertStmt),
    /// An `UPDATE` DML statement.
    Update(UpdateStmt),
    /// A `DELETE` DML statement.
    Delete(DeleteStmt),
    /// A `CREATE INDEX` DDL statement.
    CreateIndex(CreateIndexStmt),
    /// A `CREATE VIEW` DDL statement.
    CreateView(CreateViewStmt),
    /// An `ALTER VIEW` DDL statement.
    AlterView(AlterViewStmt),
    /// A `DROP VIEW` DDL statement.
    DropView(DropViewStmt),
    /// A `CREATE TYPE` DDL statement
    CreateType(CreateTypeStmt),
    /// A `CREATE DATABASE` DDL statement
    CreateDataBase(CreateDatabaseStmt),
    /// A `CREATE ROLE/USER` DDL statement
    CreateRole(CreateRoleStmt),
    /// A `CREATE TABLESPACE` DDL statement
    CreateTablespace(CreateTablespaceStmt),
    /// A `CREATE EXTENSION` DDL statement
    CreateExtension(CreateExtensionStmt),
    /// A `CREATE TRIGGER` DDL statement
    CreateTrigger(CreateTriggerStmt),
    /// A `CREATE FUNCTION` DDL statement
    CreateFunction(CreateFunctionStmt),
    /// A `CREATE Procedure` DDL statement
    CreateProcedure(CreateProcedureStmt),
    /// A transaction control statement to start a new transaction block (`BEGIN` / `BEGIN TRANSACTION`).
    Begin,
    /// A transaction control statement to commit the current transaction block (`COMMIT` / `END`).
    Commit,
    /// A transaction control statement to abort and rollback the current transaction block (`ROLLBACK` / `ABORT`).
    Rollback,
}
