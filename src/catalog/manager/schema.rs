use crate::{
    ast::CreateSchemaStmt,
    catalog::{error::CatalogError, manager::CatalogManager, objects::schema::SchemaEntry},
    common::symbol::Symbol,
};

impl CatalogManager {
    /// Executes a `CREATE SCHEMA` statement against the catalog.
    ///
    /// # Behavior
    ///
    /// - Database must exist — returns `DatabaseNotFound` if not
    /// - Already exists + `IF NOT EXISTS` → silent success
    /// - Already exists + no `IF NOT EXISTS` → error
    /// - `name` is `None` → defaults to authorization role name
    /// - `owner` not specified → resolved to `session_user`
    pub fn create_schema(
        &mut self,
        db: Symbol,
        stmt: CreateSchemaStmt,
        session_user: Symbol,
    ) -> Result<(), CatalogError> {
        let name = match stmt.name {
            Some(n) => n,
            None => stmt.authorization.unwrap_or(session_user),
        };
        let owner = stmt.authorization.unwrap_or(session_user);

        // get OID before borrowing db_entry — avoids double mutable borrow
        if !self.catalog.databases.contains_key(&db) {
            return Err(CatalogError::DatabaseNotFound(db));
        }

        let db_entry = self.catalog.databases.get(&db).unwrap();
        if db_entry.schemas.contains_key(&name) {
            if stmt.if_not_exists {
                return Ok(());
            }
            return Err(CatalogError::SchemaAlreadyExists(name));
        }

        // safe to get OID now — no active borrow on databases
        let oid = self.catalog.next_oid();

        // re-borrow mutably after OID is obtained
        let db_entry = self.catalog.databases.get_mut(&db).unwrap();
        let entry = SchemaEntry::new(oid, name, owner, db);
        db_entry.schemas.insert(name, entry);
        Ok(())
    }

    /// Returns `true` if a schema exists in the given database.
    pub fn schema_exists(&self, db: Symbol, name: Symbol) -> bool {
        self.catalog
            .databases
            .get(&db)
            .map(|d| d.schemas.contains_key(&name))
            .unwrap_or(false)
    }

    /// Looks up a schema by database and name.
    ///
    /// Returns [`CatalogError::DatabaseNotFound`] if the database does not exist.
    /// Returns [`CatalogError::SchemaNotFound`] if the schema does not exist.
    pub fn get_schema(&self, db: Symbol, name: Symbol) -> Result<&SchemaEntry, CatalogError> {
        let db_entry = self
            .catalog
            .databases
            .get(&db)
            .ok_or(CatalogError::DatabaseNotFound(db))?;

        db_entry
            .schemas
            .get(&name)
            .ok_or(CatalogError::SchemaNotFound(name))
    }

    /// Drops a schema from the catalog.
    ///
    /// Returns [`CatalogError::DatabaseNotFound`] if the database does not exist.
    /// Returns [`CatalogError::SchemaNotFound`] if the schema does not exist.
    pub fn drop_schema(
        &mut self,
        db: Symbol,
        name: Symbol,
        if_exists: bool,
    ) -> Result<(), CatalogError> {
        let db_entry = self
            .catalog
            .databases
            .get_mut(&db)
            .ok_or(CatalogError::DatabaseNotFound(db))?;

        if db_entry.schemas.remove(&name).is_none() {
            if if_exists {
                return Ok(());
            }
            return Err(CatalogError::SchemaNotFound(name));
        }
        Ok(())
    }
}
