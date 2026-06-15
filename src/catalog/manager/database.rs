use crate::{
    ast::CreateDatabaseStmt,
    catalog::{
        error::CatalogError,
        manager::CatalogManager,
        objects::{SchemaEntry, database::DatabaseEntry},
    },
    common::symbol::Symbol,
};

impl CatalogManager {
    /// Executes a `CREATE DATABASE` statement against the catalog.
    ///
    /// # Behavior
    ///
    /// - Already exists + `IF NOT EXISTS` → silent success
    /// - Already exists + no `IF NOT EXISTS` → error
    /// - `OWNER` not specified → resolved to `session_user`
    pub fn create_database(
        &mut self,
        stmt: CreateDatabaseStmt,
        session_user: Symbol,
    ) -> Result<(), CatalogError> {
        if self.catalog.databases.contains_key(&stmt.name) {
            if stmt.if_not_exists {
                return Ok(());
            }
            return Err(CatalogError::DatabaseAlreadyExists(stmt.name));
        }

        let owner = stmt.owner.unwrap_or(session_user);
        let oid = self.catalog.next_oid();

        let entry = DatabaseEntry::new(
            oid,
            stmt.name,
            owner,
            stmt.encoding,
            stmt.locale,
            stmt.tablespace,
            stmt.connection_limit,
        );

        self.catalog.databases.insert(stmt.name, entry);

        // Every database gets a default `public` schema, mirroring the
        // `public/` directory storage creates alongside the database.
        let public = self.interner.intern("public");
        let public_oid = self.catalog.next_oid();
        let public_schema = SchemaEntry::new(public_oid, public, owner, stmt.name);
        self.catalog
            .databases
            .get_mut(&stmt.name)
            .unwrap()
            .schemas
            .insert(public, public_schema);
        Ok(())
    }

    /// Returns `true` if a database with the given name exists.
    pub fn database_exists(&self, name: Symbol) -> bool {
        self.catalog.databases.contains_key(&name)
    }

    /// Looks up a database by name.
    ///
    /// Returns [`CatalogError::DatabaseNotFound`] if it does not exist.
    pub fn get_database(&self, name: Symbol) -> Result<&DatabaseEntry, CatalogError> {
        self.catalog
            .databases
            .get(&name)
            .ok_or(CatalogError::DatabaseNotFound(name))
    }

    /// Drops a database from the catalog.
    ///
    /// Caller must ensure no active connections exist before calling.
    pub fn drop_database(&mut self, name: Symbol, if_exists: bool) -> Result<(), CatalogError> {
        if self.catalog.databases.remove(&name).is_none() {
            if if_exists {
                return Ok(());
            }
            return Err(CatalogError::DatabaseNotFound(name));
        }
        Ok(())
    }
}
