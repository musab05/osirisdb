use crate::ast::*;

/// Represents a DDL statement to quickly empty tables (`TRUNCATE TABLE`).
#[derive(Debug, Clone, PartialEq)]
pub struct TruncateStmt {
    /// The qualified names of the tables to truncate.
    pub tables: Vec<ObjectName>,
    /// If `true`, sequence generators owned by columns of the tables are restarted (`RESTART IDENTITY`).
    pub restart_identity: bool,
    /// Optional cascade/restrict behavior for dropping associated data dependency objects.
    pub behaviour: Option<DropBehavior>,
}
