use crate::common::symbol::Symbol;

/// The catalog's runtime representation of a database.
///
/// Unlike [`CreateDatabaseStmt`] which is a parse-time snapshot of what
/// the user wrote, `DatabaseEntry` is what lives in the catalog after the
/// statement has been validated and executed. It holds the resolved,
/// authoritative state of the database.
///
/// Key differences from the AST node:
/// - No `if_not_exists` — that flag is consumed during execution, irrelevant after
/// - `owner` is required — resolved to a default (session user) if not specified
/// - Adds `oid` — internal unique identifier used by all catalog lookups
#[derive(Debug, Clone, PartialEq)]
pub struct DatabaseEntry {
    /// Internal unique identifier for this database.
    ///
    /// Used as a stable key for catalog lookups, foreign references from
    /// schemas and tablespaces, and on-disk file/directory naming.
    /// Never changes after creation — even if the database is renamed.
    pub oid: u32,

    /// The name of the database.
    ///
    /// Stored as a [`Symbol`] for O(1) equality checks during catalog
    /// lookups. Resolve via the interner for display.
    pub name: Symbol,

    /// The role that owns this database.
    ///
    /// Always present — resolved to the session user at creation time
    /// if `OWNER` was not specified in the `CREATE DATABASE` statement.
    pub owner: Symbol,

    /// Character encoding for this database (e.g. `UTF8`, `SQL_ASCII`).
    ///
    /// `None` means the cluster default encoding is inherited.
    pub encoding: Option<Symbol>,

    /// Locale setting controlling collation and character classification.
    ///
    /// Maps to both `LC_COLLATE` and `LC_CTYPE`. `None` means the
    /// cluster default locale is inherited.
    pub locale: Option<Symbol>,

    /// The tablespace where this database's objects are stored by default.
    ///
    /// `None` means the cluster default tablespace (`pg_default` equivalent).
    pub tablespace: Option<Symbol>,

    /// Maximum number of concurrent connections allowed to this database.
    ///
    /// `None` means unlimited. `-1` is also treated as unlimited.
    /// Enforced at connection time by the session manager.
    pub connection_limit: Option<i64>,

    /// Whether this database allows connections.
    ///
    /// `false` is used for template databases that exist only to be
    /// cloned — direct connections to them are rejected.
    /// Defaults to `true` for all user-created databases.
    pub allow_connections: bool,

    /// Whether this database can be used as a template for `CREATE DATABASE ... TEMPLATE`.
    ///
    /// Defaults to `false` for user-created databases.
    /// Template databases are typically locked and cannot be modified.
    pub is_template: bool,
}

impl DatabaseEntry {
    /// Creates a new `DatabaseEntry` from a validated `CREATE DATABASE` statement.
    ///
    /// `oid` is assigned by the catalog manager from a monotonic counter.
    /// `resolved_owner` is the session user symbol if no `OWNER` was specified.
    pub fn new(
        oid: u32,
        name: Symbol,
        resolved_owner: Symbol,
        encoding: Option<Symbol>,
        locale: Option<Symbol>,
        tablespace: Option<Symbol>,
        connection_limit: Option<i64>,
    ) -> Self {
        Self {
            oid,
            name,
            owner: resolved_owner,
            encoding,
            locale,
            tablespace,
            connection_limit,
            allow_connections: true,  // always true for user-created databases
            is_template: false,       // always false for user-created databases
        }
    }
}