use crate::{
    ast::CreateTableStmt,
    binder::{BindError, Binder, bound::BoundCreateTableStmt},
    catalog::objects::ColumnEntry,
    common::symbol::Symbol,
};

impl<'c> Binder<'c> {
    /// Binds a `CREATE TABLE` statement.
    ///
    /// # Validation performed
    ///
    /// 1. Confirms the target schema exists in the catalog.
    /// 2. If `IF NOT EXISTS` is false, confirms the table does not already exist
    ///    in the target database and schema.
    ///
    /// # Resolution performed
    ///
    /// - Resolves the schema and table name. If the schema is not specified in
    ///   the table path, it defaults to the `default_schema`.
    /// - Maps AST column definitions into [`ColumnEntry`] representations.
    ///
    /// # Errors
    ///
    /// Returns [`BindError`] if any validation fails.
    pub fn bind_create_table(
        &self,
        db: Symbol,
        default_schema: Symbol,
        stmt: CreateTableStmt,
    ) -> Result<BoundCreateTableStmt, BindError> {
        let (schema, name) = stmt.name.resolve_schema_table(default_schema);

        if !self.catalog.schema_exists(db, schema) {
            return Err(BindError::SchemaNotFound(schema));
        }

        if !stmt.if_not_exist && self.catalog.table_exists(db, schema, name) {
            return Err(BindError::TableAlreadyExists(name));
        }

        let columns = stmt
            .columns
            .into_iter()
            .map(|c| ColumnEntry {
                name: c.name,
                data_type: c.data_type,
            })
            .collect();

        Ok(BoundCreateTableStmt {
            db,
            schema,
            name,
            columns,
            if_not_exists: stmt.if_not_exist,
        })
    }
}
