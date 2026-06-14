use crate::common::symbol::Symbol;

/// The result of a successfully executed statement.
///
/// Each variant corresponds to a statement type and carries enough
/// information for the client to display a confirmation message
/// (e.g. `CREATE DATABASE` in psql).
///
/// Analogous to PostgreSQL's command tags.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionResult {
    /// A `CREATE DATABASE` statement completed successfully.
    /// Carries the name of the created database.
    DatabaseCreated { name: Symbol },

    /// A `DROP DATABASE` statement completed successfully.
    DatabaseDropped { name: Symbol },

    /// A `CREATE SCHEMA` statement completed successfully.
    /// Carries the name of the created schema.
    SchemaCreated { name: Symbol },

    /// A `CREATE TABLE` statement completed successfully.
    /// Carries the name of the created table.
    TableCreated { name: Symbol },
}

impl ExecutionResult {
    /// Returns a PostgreSQL-style command tag string.
    ///
    /// This is what gets displayed to the user after a successful statement:
    /// ```text
    /// CREATE DATABASE
    /// DROP DATABASE
    /// ```
    pub fn command_tag(&self) -> &'static str {
        match self {
            ExecutionResult::DatabaseCreated { .. } => "CREATE DATABASE",
            ExecutionResult::DatabaseDropped { .. } => "DROP DATABASE",
            ExecutionResult::SchemaCreated { .. } => "CREATE SCHEMA",
            ExecutionResult::TableCreated { .. } => "CREATE TABLE",
        }
    }
}
