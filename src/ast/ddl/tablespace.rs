use crate::{ast::*, common::symbol::Symbol};

/// Represents filter or parameter options for a `CREATE TABLESPACE` statement.
///
/// This AST node captures the parameters for creating a new tablespace,
/// corresponding to `CREATE TABLESPACE name [ OWNER user_name ] LOCATION 'directory' [ WITH ( tablespace_option = value [, ... ] ) ]`.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateTablespaceStmt {
    /// The name of the tablespace to create.
    pub name: Symbol,
    /// Optional role name of the user who will own the new tablespace.
    pub owner: Option<Symbol>,
    /// The directory location where the tablespace will be stored.
    pub location: Symbol,
    /// Optional tablespace configuration options specified in the `WITH` clause.
    pub options: Vec<SqlOption>,
}
