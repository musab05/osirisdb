use crate::{
    ast::CreateDatabaseStmt,
    binder::{bound::database::BoundCreateDatabaseStmt, error::BindError},
    catalog::CatalogManager,
    common::symbol::Symbol,
};

/// The binder — validates AST nodes against the catalog and produces
/// fully resolved bound nodes ready for the executor.
///
/// # Responsibilities
///
/// - Name resolution — confirms referenced objects exist in the catalog
/// - Default resolution — fills in omitted values (e.g. owner → session user)
/// - Semantic validation — catches errors that the parser cannot
///   (e.g. unknown role, invalid connection limit)
///
/// # What the binder does NOT do
///
/// - Modify the catalog — that is the executor's job
/// - Parse SQL — that is the parser's job
/// - Optimize — that is the planner's job
///
/// # Relationship to CatalogManager
///
/// The binder holds a shared read-only reference to the catalog.
/// Only the executor writes to the catalog via `CatalogManager`.
pub struct Binder<'c> {
    /// Read-only view of the catalog for name resolution.
    catalog: &'c CatalogManager,

    /// The current session user — used as default owner when OWNER is omitted.
    session_user: Symbol,
}

impl<'c> Binder<'c> {
    /// Creates a new `Binder` for the current session.
    ///
    /// `catalog` is borrowed read-only — the binder never modifies it.
    /// `session_user` is the symbol of the currently connected role,
    /// used to resolve ownership defaults.
    pub fn new(catalog: &'c CatalogManager, session_user: Symbol) -> Self {
        Self {
            catalog,
            session_user,
        }
    }

    /// Binds a `CREATE DATABASE` statement.
    ///
    /// # Validation performed
    ///
    /// 1. If `IF NOT EXISTS` is false — confirms the database does not already exist
    /// 2. If `OWNER` is specified — confirms the role exists in the catalog
    /// 3. If `TABLESPACE` is specified — confirms the tablespace exists in the catalog
    /// 4. If `CONNECTION LIMIT` is specified — validates value is >= -1
    ///
    /// # Resolution performed
    ///
    /// - `owner` — defaults to `session_user` if not specified
    ///
    /// # Errors
    ///
    /// Returns [`BindError`] if any validation fails.
    pub fn bind_create_database(
        &self,
        stmt: CreateDatabaseStmt,
    ) -> Result<BoundCreateDatabaseStmt, BindError> {
        // 1. Check database does not already exist
        //    Skip if IF NOT EXISTS — executor will handle the silent success
        if !stmt.if_not_exists && self.catalog.database_exists(stmt.name) {
            return Err(BindError::DatabaseAlreadyExists(stmt.name));
        }

        // 2. Validate owner role exists if specified
        //    We only validate — resolution happens next
        if let Some(owner) = stmt.owner {
            if !self.catalog.role_exists(owner) {
                return Err(BindError::RoleNotFound(owner));
            }
        }

        // 3. Resolve owner — use session user if not specified
        let owner = stmt.owner.unwrap_or(self.session_user);

        // 4. Validate tablespace exists if specified
        if let Some(ts) = stmt.tablespace {
            if !self.catalog.tablespace_exists(ts) {
                return Err(BindError::TablespaceNotFound(ts));
            }
        }

        // 5. Validate connection limit
        if let Some(limit) = stmt.connection_limit {
            if limit < -1 {
                return Err(BindError::InvalidConnectionLimit(limit));
            }
        }

        Ok(BoundCreateDatabaseStmt {
            name: stmt.name,
            if_not_exists: stmt.if_not_exists,
            owner,
            encoding: stmt.encoding,
            locale: stmt.locale,
            tablespace: stmt.tablespace,
            connection_limit: stmt.connection_limit,
        })
    }
}
