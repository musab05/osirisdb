use crate::{
    ast::CreateDatabaseStmt,
    catalog::{catalog::Catalog, error::CatalogError, objects::database::DatabaseEntry},
    common::{interner::Interner, symbol::Symbol},
};

/// The entry point for all catalog mutations.
///
/// `CatalogManager` owns both the [`Catalog`] (data) and the [`Interner`]
/// (name resolution). It is the only component allowed to write to the
/// catalog — all other layers get a read-only reference via [`CatalogManager::catalog`].
///
/// # Why separate from `Catalog`?
///
/// - `Catalog` is pure data — no logic, easy to snapshot for transactions
/// - `CatalogManager` is behavior — validation, OID assignment, persistence hooks
/// - Makes testing straightforward — data and logic are independently testable
pub struct CatalogManager {
    /// The live catalog state.
    pub catalog: Catalog,

    /// Shared interner — names interned during parsing are resolved here.
    ///
    /// The interner is moved into the manager after parsing completes so
    /// that catalog operations can intern new names (e.g. system defaults)
    /// using the same symbol space as the parser.
    pub interner: Interner,
}

impl CatalogManager {
    /// Creates a new `CatalogManager` with an empty catalog.
    pub fn new(interner: Interner) -> Self {
        Self {
            catalog: Catalog::new(),
            interner,
        }
    }

    /// Executes a `CREATE DATABASE` statement against the catalog.
    ///
    /// # Behavior
    ///
    /// - If the database already exists and `IF NOT EXISTS` is set → silent success
    /// - If the database already exists and `IF NOT EXISTS` is not set → error
    /// - If `OWNER` was not specified → resolved to `session_user` (passed by caller)
    ///
    /// # Errors
    ///
    /// Returns [`CatalogError::DatabaseAlreadyExists`] if the database exists
    /// and `if_not_exists` is `false`.
    pub fn create_database(
        &mut self,
        stmt: CreateDatabaseStmt,
        session_user: Symbol,
    ) -> Result<(), CatalogError> {
        // Check existence first
        if self.catalog.databases.contains_key(&stmt.name) {
            if stmt.if_not_exists {
                // IF NOT EXISTS — silently succeed, nothing to do
                return Ok(());
            }
            return Err(CatalogError::DatabaseAlreadyExists(stmt.name));
        }

        // Resolve owner — fall back to session user if not specified
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

        Ok(())
    }

    /// Returns `true` if a database with the given name exists in the catalog.
    pub fn database_exists(&self, name: Symbol) -> bool {
        self.catalog.databases.contains_key(&name)
    }

    /// Looks up a database by name.
    ///
    /// Returns a reference to the [`DatabaseEntry`] or
    /// [`CatalogError::DatabaseNotFound`] if it does not exist.
    pub fn get_database(&self, name: Symbol) -> Result<&DatabaseEntry, CatalogError> {
        self.catalog
            .databases
            .get(&name)
            .ok_or(CatalogError::DatabaseNotFound(name))
    }

    /// Drops a database from the catalog.
    ///
    /// Returns [`CatalogError::DatabaseNotFound`] if the database does not exist.
    /// The caller is responsible for checking that no active connections exist
    /// before calling this.
    pub fn drop_database(&mut self, name: Symbol, if_exists: bool) -> Result<(), CatalogError> {
        if self.catalog.databases.remove(&name).is_none() {
            if if_exists {
                return Ok(()); // DROP DATABASE IF EXISTS — silent success
            }
            return Err(CatalogError::DatabaseNotFound(name));
        }
        Ok(())
    }
}