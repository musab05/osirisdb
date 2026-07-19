use crate::{
    ast::session::database::UseDatabaseStmt,
    binder::{BindError, Binder, bound::database::BoundUseDatabaseStmt},
};

impl<'c> Binder<'c> {
    pub fn bind_use_database(
        &self,
        stmt: UseDatabaseStmt,
    ) -> Result<BoundUseDatabaseStmt, BindError> {
        // Validate againts the catalog
        if !self.catalog.database_exists(stmt.database_name) {
            return Err(BindError::DatabaseNotFound(stmt.database_name));
        }

        // Return a bound plan
        Ok(BoundUseDatabaseStmt {
            database_name: stmt.database_name,
        })
    }
}
