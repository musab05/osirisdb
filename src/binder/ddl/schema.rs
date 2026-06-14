use crate::{
    ast::CreateSchemaStmt,
    binder::{BindError, Binder, bound::BoundCreateSchemaStmt},
    common::symbol::Symbol,
};

impl<'c> Binder<'c> {
    /// Binds a `CREATE SCHEMA` statement.
    ///
    /// # Validation performed
    ///
    /// 1. Confirms the target database exists in the catalog
    /// 2. If `AUTHORIZATION` is specified — confirms the role exists in the catalog
    /// 3. If `IF NOT EXISTS` is false — confirms the schema does not already exist
    ///    in the target database
    ///
    /// # Resolution performed
    ///
    /// - `name` — if not specified, defaults to the resolved `authorization` role
    /// - `owner` — defaults to `session_user` if `AUTHORIZATION` not specified
    ///
    /// # Errors
    ///
    /// Returns [`BindError`] if any validation fails.
    pub fn bind_create_schema(
        &self,
        db: Symbol,
        stmt: CreateSchemaStmt,
    ) -> Result<BoundCreateSchemaStmt, BindError> {
        // 1. Confirm target database exists
        if !self.catalog.database_exists(db) {
            return Err(BindError::DatabaseNotFound(db));
        }

        // 2. Validate authorization role exists if specified
        if let Some(auth) = stmt.authorization {
            if !self.catalog.role_exists(auth) {
                return Err(BindError::RoleNotFound(auth));
            }
        }

        // 3. Resolve schema name — defaults to authorization role, else session user
        let name = match stmt.name {
            Some(n) => n,
            None => stmt.authorization.unwrap_or(self.session_user),
        };

        // 4. Check schema does not already exist
        //    Skip if IF NOT EXISTS — executor will handle the silent success
        if !stmt.if_not_exists && self.catalog.schema_exists(db, name) {
            return Err(BindError::SchemaAlreadyExists(name));
        }

        Ok(BoundCreateSchemaStmt {
            name: Some(name),
            authorization: stmt.authorization,
            if_not_exists: stmt.if_not_exists,
        })
    }
}