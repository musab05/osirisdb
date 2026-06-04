
/// Represents a `CREATE DATABASE` statement.
///
/// This AST node captures the parameters for creating a new database,
/// corresponding to `CREATE DATABASE [IF NOT EXISTS] name [OWNER owner] [ENCODING encoding] ...`
#[derive(Debug, Clone, PartialEq)]
pub struct CreateDatabaseStmt {
    /// The name of the database to create.
    pub name: String,
    /// If true, do not throw an error if a database with the same name already exists (`IF NOT EXISTS`).
    pub if_not_exists: bool,
    /// Optional role name of the user who will own the new database.
    pub owner: Option<String>,
    /// Optional character encoding for the new database (e.g., "UTF8").
    pub encoding: Option<String>,
    /// Optional locale (collation and character classification) for the database.
    pub locale: Option<String>,
    /// Optional name of the tablespace associated with the new database.
    pub tablespace: Option<String>,
    /// Optional maximum number of concurrent connections allowed.
    pub connection_limit: Option<i64>,
}
