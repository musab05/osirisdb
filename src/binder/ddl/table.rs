use crate::{
    ast::{ColumnConstraint, CreateTableStmt, TableConstraint},
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
    /// 3. Confirms no column name is defined more than once.
    /// 4. For each table-level constraint, confirms every referenced column
    ///    name is defined on this table.
    /// 5. Confirms at most one `PRIMARY KEY` is specified across all
    ///    column-level and table-level constraints combined.
    ///
    /// # Resolution performed
    ///
    /// - Resolves the schema and table name. If the schema is not specified in
    ///   the table path, it defaults to `default_schema`.
    /// - Maps AST column definitions into [`ColumnEntry`] representations,
    ///   folding each column's [`ColumnConstraint`]s into resolved flags:
    ///   - `NULL` / `NOT NULL` → `nullable`
    ///   - `DEFAULT <expr>` → `default`
    ///   - `UNIQUE` → `is_unique`
    ///   - `PRIMARY KEY` → `is_primary_key` (and forces `nullable = false`)
    ///   - `CHECK`, `REFERENCES`, `AUTO_INCREMENT` are not yet enforced —
    ///     see inline notes below.
    /// - Folds simple table-level `PRIMARY KEY (col)` / `UNIQUE (col)`
    ///   constraints (single column) into the matching [`ColumnEntry`] flags.
    /// - Composite `PRIMARY KEY`/`UNIQUE` (multiple columns), `CHECK`, and
    ///   `FOREIGN KEY` constraints are validated (referenced columns must
    ///   exist) and carried through unchanged into
    ///   [`crate::catalog::objects::TableEntry::constraints`].
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

        // 1. Target schema must exist
        if !self.catalog.schema_exists(db, schema) {
            return Err(BindError::SchemaNotFound(schema));
        }

        // 2. Table must not already exist (unless IF NOT EXISTS)
        if !stmt.if_not_exist && self.catalog.table_exists(db, schema, name) {
            return Err(BindError::TableAlreadyExists(name));
        }

        // Tracks whether a PRIMARY KEY has been seen yet (column-level or
        // table-level) — used to enforce at most one across the whole table.
        let mut has_primary_key = false;

        // 3. Build ColumnEntry list, folding column-level constraints into flags
        let mut columns: Vec<ColumnEntry> = Vec::with_capacity(stmt.columns.len());
        for col in stmt.columns {
            // Duplicate column name check
            if columns.iter().any(|c| c.name == col.name) {
                return Err(BindError::DuplicateColumnName(col.name));
            }

            let mut nullable = true; // SQL default: columns are nullable
            let mut default = None;
            let mut is_unique = false;
            let mut is_primary_key = false;

            for constraint in col.constraints {
                match constraint {
                    ColumnConstraint::Null => {
                        nullable = true;
                    }
                    ColumnConstraint::NotNull => {
                        nullable = false;
                    }
                    ColumnConstraint::Default(expr) => {
                        default = Some(expr);
                    }
                    ColumnConstraint::Unique => {
                        is_unique = true;
                    }
                    ColumnConstraint::PrimaryKey => {
                        if has_primary_key {
                            return Err(BindError::MultiplePrimaryKeys);
                        }
                        has_primary_key = true;
                        is_primary_key = true;
                        // PRIMARY KEY implies NOT NULL
                        nullable = false;
                    }
                    // TODO: CHECK requires an expression evaluator to enforce
                    // at INSERT/UPDATE time — parsed and accepted, but not
                    // validated or stored on the column. Once table-level
                    // CHECK support lands (TableConstraint::Check below),
                    // column-level CHECK should be normalized into the same
                    // representation rather than dropped silently.
                    ColumnConstraint::Check(_) => {}
                    // TODO: REFERENCES requires cross-table catalog lookups
                    // (the foreign table must exist and have matching
                    // columns) — deferred until multi-table validation is
                    // supported. Parsed and accepted, not stored or enforced.
                    ColumnConstraint::References { .. } => {}
                    // TODO: AUTO_INCREMENT requires sequence/identity support
                    // in storage — deferred. Parsed and accepted, not stored
                    // or enforced.
                    ColumnConstraint::AutoIncrement => {}
                }
            }

            columns.push(ColumnEntry {
                name: col.name,
                data_type: col.data_type,
                nullable,
                default,
                is_unique,
                is_primary_key,
            });
        }

        // 4. Process table-level constraints
        let mut table_constraints = Vec::new();
        for constraint in stmt.constraints {
            match &constraint {
                TableConstraint::PrimaryKey { columns: cols, .. } => {
                    // Every referenced column must exist on this table
                    for col_name in cols {
                        if !columns.iter().any(|c| c.name == *col_name) {
                            return Err(BindError::ColumnNotFound(*col_name));
                        }
                    }

                    if has_primary_key {
                        return Err(BindError::MultiplePrimaryKeys);
                    }
                    has_primary_key = true;

                    if cols.len() == 1 {
                        // Single-column PRIMARY KEY (col) — fold into the
                        // matching ColumnEntry rather than keeping it as a
                        // separate table-level constraint.
                        let col_name = cols[0];
                        let entry = columns
                            .iter_mut()
                            .find(|c| c.name == col_name)
                            .expect("column existence checked above");
                        entry.is_primary_key = true;
                        entry.nullable = false;
                    } else {
                        // Composite PRIMARY KEY — mark each participating
                        // column and keep the constraint for the full
                        // composite definition.
                        for col_name in cols {
                            let entry = columns
                                .iter_mut()
                                .find(|c| c.name == *col_name)
                                .expect("column existence checked above");
                            entry.is_primary_key = true;
                            entry.nullable = false;
                        }
                        table_constraints.push(constraint);
                    }
                }

                TableConstraint::Unique { columns: cols, .. } => {
                    for col_name in cols {
                        if !columns.iter().any(|c| c.name == *col_name) {
                            return Err(BindError::ColumnNotFound(*col_name));
                        }
                    }

                    if cols.len() == 1 {
                        // Single-column UNIQUE (col) — fold into the
                        // matching ColumnEntry.
                        let col_name = cols[0];
                        let entry = columns
                            .iter_mut()
                            .find(|c| c.name == col_name)
                            .expect("column existence checked above");
                        entry.is_unique = true;
                    } else {
                        // Composite UNIQUE spans multiple columns — cannot
                        // be represented by a single-column flag, keep as
                        // a table-level constraint.
                        table_constraints.push(constraint);
                    }
                }

                // TODO: CHECK requires an expression evaluator to enforce —
                // referenced columns aren't validated yet since Expr column
                // references aren't resolved at this layer. Carried through
                // unchanged for now.
                TableConstraint::Check { .. } => {
                    table_constraints.push(constraint);
                }

                TableConstraint::ForeignKey {
                    columns: cols,
                    referred_columns: _,
                    ..
                } => {
                    // Local columns must exist on this table.
                    for col_name in cols {
                        if !columns.iter().any(|c| c.name == *col_name) {
                            return Err(BindError::ColumnNotFound(*col_name));
                        }
                    }
                    // TODO: validating `foreign_table` exists and
                    // `referred_columns` exist on it requires cross-table
                    // catalog lookups — deferred. Carried through unchanged.
                    table_constraints.push(constraint);
                }
            }
        }

        Ok(BoundCreateTableStmt {
            db,
            schema,
            name,
            columns,
            constraints: table_constraints,
            if_not_exists: stmt.if_not_exist,
        })
    }
}
