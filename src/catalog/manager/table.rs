use crate::{
    catalog::{
        CatalogError, CatalogManager,
        objects::{TableEntry, column::ColumnEntry},
    },
    common::symbol::Symbol,
};

impl CatalogManager {
    /// Executes a `CREATE TABLE` statement against the catalog.
    ///
    /// # Behavior
    ///
    /// - Database must exist — returns `DatabaseNotFound` if not.
    /// - Schema must exist — returns `SchemaNotFound` if not.
    /// - Table already exists + `if_not_exists` → silent success.
    /// - Table already exists + no `if_not_exists` → returns `DatabaseAlreadyExists`.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::DatabaseNotFound`], [`CatalogError::SchemaNotFound`], or
    /// [`CatalogError::DatabaseAlreadyExists`] if validation fails.
    pub fn create_table(
        &mut self,
        db: Symbol,
        schema: Symbol,
        name: Symbol,
        columns: Vec<ColumnEntry>,
        if_not_exists: bool,
    ) -> Result<(), CatalogError> {
        let db_entry = self
            .catalog
            .databases
            .get(&db)
            .ok_or(CatalogError::DatabaseNotFound(db))?;

        let schema_entry = db_entry
            .schemas
            .get(&schema)
            .ok_or(CatalogError::SchemaNotFound(schema))?;

        if schema_entry.tables.contains_key(&name) {
            if if_not_exists {
                return Ok(());
            }
            return Err(CatalogError::DatabaseAlreadyExists(name));
        }

        let oid = self.catalog.next_oid();

        let db_entry = self.catalog.databases.get_mut(&db).unwrap();
        let schema_entry = db_entry.schemas.get_mut(&schema).unwrap();
        let entry = TableEntry::new(oid, name, columns);
        schema_entry.tables.insert(name, entry);
        Ok(())
    }

    /// Returns `true` if a table exists in the given database and schema.
    pub fn table_exists(&self, db: Symbol, schema: Symbol, name: Symbol) -> bool {
        self.catalog
            .databases
            .get(&db)
            .and_then(|d| d.schemas.get(&schema))
            .map(|s| s.tables.contains_key(&name))
            .unwrap_or(false)
    }

    /// Looks up a table by database, schema, and name.
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::DatabaseNotFound`] if the database does not exist.
    /// Returns [`CatalogError::SchemaNotFound`] if the schema does not exist.
    /// Returns [`CatalogError::TableNotFound`] if the table does not exist.
    pub fn get_table(
        &self,
        db: Symbol,
        schema: Symbol,
        name: Symbol,
    ) -> Result<&TableEntry, CatalogError> {
        let db_entry = self
            .catalog
            .databases
            .get(&db)
            .ok_or(CatalogError::DatabaseNotFound(db))?;
        let schema_entry = db_entry
            .schemas
            .get(&schema)
            .ok_or(CatalogError::SchemaNotFound(schema))?;
        schema_entry
            .tables
            .get(&name)
            .ok_or(CatalogError::TableNotFound(name))
    }
}
