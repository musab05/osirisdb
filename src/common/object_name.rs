use crate::{ast::ObjectName, common::symbol::Symbol};

impl ObjectName {
    pub fn resolve_schema_table(&self, default_schema: Symbol) -> (Symbol, Symbol) {
        match self.0.as_slice() {
            [table] => (default_schema, *table),
            [schema, table] => (*schema, *table),
            _ => panic!("unsupported: db.schema.table not yet supported"),
        }
    }
}
