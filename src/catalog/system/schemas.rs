use crate::{ast::DataType, catalog::objects::ColumnEntry, common::Interner};

fn col(interner: &mut Interner, name: &str, data_type: DataType, nullable: bool) -> ColumnEntry {
    ColumnEntry {
        name: interner.intern(name),
        data_type,
        nullable,
        default: None,
        is_unique: false,
        is_primary_key: false,
    }
}
pub fn osiris_database_schema(interner: &mut Interner) -> Vec<ColumnEntry> {
    vec![
        col(interner, "oid", DataType::Int, false),
        col(interner, "name", DataType::VarChar(None), false),
        col(interner, "owner", DataType::VarChar(None), false),
        col(interner, "encoding", DataType::VarChar(None), true),
        col(interner, "locale", DataType::VarChar(None), true),
        col(interner, "tablespace", DataType::VarChar(None), true),
        col(interner, "conn_limit", DataType::Int, true),
        col(interner, "allow_conn", DataType::Boolean, false),
        col(interner, "is_template", DataType::Boolean, false),
    ]
}

pub fn osiris_namespace_schema(interner: &mut Interner) -> Vec<ColumnEntry> {
    vec![
        col(interner, "oid", DataType::Int, false),
        col(interner, "name", DataType::VarChar(None), false),
        col(interner, "owner", DataType::VarChar(None), false),
        col(interner, "database", DataType::VarChar(None), false),
    ]
}

pub fn osiris_class_schema(interner: &mut Interner) -> Vec<ColumnEntry> {
    vec![
        col(interner, "oid", DataType::Int, false),
        col(interner, "name", DataType::VarChar(None), false),
        col(interner, "schema", DataType::VarChar(None), false),
        col(interner, "database", DataType::VarChar(None), false),
    ]
}

pub fn osiris_attribute_schema(interner: &mut Interner) -> Vec<ColumnEntry> {
    vec![
        col(interner, "table_oid", DataType::Int, false),
        col(interner, "col_index", DataType::Int, false),
        col(interner, "name", DataType::VarChar(None), false),
        col(interner, "col_type", DataType::VarChar(None), false),
        col(interner, "nullable", DataType::Boolean, false),
        col(interner, "is_unique", DataType::Boolean, false),
        col(interner, "is_pk", DataType::Boolean, false),
    ]
}
