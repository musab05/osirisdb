use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::{
    ast::{DataType, Value},
    catalog::{
        objects::{ColumnEntry, DatabaseEntry, SchemaEntry, TableEntry},
        system::schemas::{
            osiris_attribute_schema, osiris_class_schema, osiris_database_schema,
            osiris_namespace_schema,
        },
    },
    common::interner::Interner,
    storage::{StorageError, TableHeap, file::HeapFile, pool::BufferPool},
};

const SYSTEM_DIR: &str = "_system";
const POOL_CAPACITY: usize = 16;

/// Manages the four on-disk system catalog tables:
/// `osiris_database`, `osiris_namespace`, `osiris_class`, `osiris_attribute`.
///
/// Writes a row on every DDL operation and reads all rows at startup
/// to reconstruct the in-memory catalog.
pub struct SystemCatalog {
    data_dir: PathBuf,
}

impl SystemCatalog {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    /// Creates `_system/` and initialises the four empty heap files.
    /// Called once on first startup.
    pub fn init(&self) -> Result<(), StorageError> {
        let dir = self.data_dir.join(SYSTEM_DIR);
        if !dir.exists() {
            std::fs::create_dir_all(&dir).map_err(|e| StorageError::io(&dir, e))?;
        }
        for name in [
            "osiris_database",
            "osiris_namespace",
            "osiris_class",
            "osiris_attribute",
        ] {
            let path = dir.join(format!("{}.dat", name));
            if !path.exists() {
                std::fs::File::create(&path).map_err(|e| StorageError::io(&path, e))?;
            }
        }
        Ok(())
    }

    fn heap(&self, table: &str) -> Result<TableHeap, StorageError> {
        let path = self
            .data_dir
            .join(SYSTEM_DIR)
            .join(format!("{}.dat", table));
        let hf = HeapFile::open(path)?;
        Ok(TableHeap::from_buffer_pool(Arc::new(Mutex::new(
            BufferPool::new(hf, POOL_CAPACITY),
        ))))
    }

    // ── Writers ──────────────────────────────────────────────────────────────

    pub fn write_database(
        &self,
        entry: &DatabaseEntry,
        interner: &mut Interner,
    ) -> Result<(u32, u16), StorageError> {
        let schema = osiris_database_schema(interner);

        let row = vec![
            Value::Int(entry.oid as i64),
            Value::String(entry.name),
            Value::String(entry.owner),
            entry
                .encoding
                .map(|s| Value::String(s))
                .unwrap_or(Value::Null),
            entry
                .locale
                .map(|s| Value::String(s))
                .unwrap_or(Value::Null),
            entry
                .tablespace
                .map(|s| Value::String(s))
                .unwrap_or(Value::Null),
            entry
                .connection_limit
                .map(Value::Int)
                .unwrap_or(Value::Null),
            Value::Boolean(entry.allow_connections),
            Value::Boolean(entry.is_template),
        ];

        self.heap("osiris_database")?
            .insert_tuple(&schema, &row, interner)
    }

    pub fn write_schema(
        &self,
        entry: &SchemaEntry,
        interner: &mut Interner,
    ) -> Result<(u32, u16), StorageError> {
        let schema = osiris_namespace_schema(interner);
        let row = vec![
            Value::Int(entry.oid as i64),
            Value::String(entry.name),
            Value::String(entry.owner),
            Value::String(entry.database),
        ];
        self.heap("osiris_namespace")?
            .insert_tuple(&schema, &row, interner)
    }

    pub fn write_table(
        &self,
        db: &str,
        schema_name: &str,
        entry: &TableEntry,
        interner: &mut Interner,
    ) -> Result<(), StorageError> {
        let class_schema = osiris_class_schema(interner);
        let class_row = vec![
            Value::Int(entry.oid as i64),
            Value::String(entry.name),
            Value::String(interner.intern(schema_name)),
            Value::String(interner.intern(db)),
        ];
        self.heap("osiris_class")?
            .insert_tuple(&class_schema, &class_row, interner)?;

        let attr_schema = osiris_attribute_schema(interner);
        let mut attr_heap = self.heap("osiris_attribute")?;
        for (i, col) in entry.columns.iter().enumerate() {
            let type_str = format_data_type(&col.data_type);
            let type_sym = interner.intern(&type_str);
            let attr_row = vec![
                Value::Int(entry.oid as i64),
                Value::Int(i as i64),
                Value::String(col.name),
                Value::String(type_sym),
                Value::Boolean(col.nullable),
                Value::Boolean(col.is_unique),
                Value::Boolean(col.is_primary_key),
            ];
            attr_heap.insert_tuple(&attr_schema, &attr_row, interner)?;
        }
        Ok(())
    }

    // ── Startup recovery ─────────────────────────────────────────────────────

    /// Reads all four system tables and returns reconstructed entries.
    pub fn load_all(&self, interner: &mut Interner) -> Result<Vec<DatabaseEntry>, StorageError> {
        // load databases
        let db_schema = osiris_database_schema(interner);
        let db_rows = self.heap("osiris_database")?.scan(&db_schema, interner)?;

        let mut databases: Vec<DatabaseEntry> = db_rows
            .into_iter()
            .map(|row| {
                DatabaseEntry::new(
                    row[0].as_int() as u32,
                    row[1].as_sym(),
                    row[2].as_sym(),
                    row[3].as_sym_opt(),
                    row[4].as_sym_opt(),
                    row[5].as_sym_opt(),
                    row[6].as_int_opt(),
                )
            })
            .collect();

        // load namespaces
        let ns_schema = osiris_namespace_schema(interner);
        let ns_rows = self.heap("osiris_namespace")?.scan(&ns_schema, interner)?;
        for row in ns_rows {
            let db_sym = row[3].as_sym();
            if let Some(db) = databases.iter_mut().find(|d| d.name == db_sym) {
                let entry = SchemaEntry::new(
                    row[0].as_int() as u32,
                    row[1].as_sym(),
                    row[2].as_sym(),
                    db_sym,
                );
                db.schemas.insert(entry.name, entry);
            }
        }

        // load tables
        let class_schema = osiris_class_schema(interner);
        let class_rows = self.heap("osiris_class")?.scan(&class_schema, interner)?;

        let attr_schema = osiris_attribute_schema(interner);
        let attr_rows = self
            .heap("osiris_attribute")?
            .scan(&attr_schema, interner)?;

        for row in class_rows {
            let table_oid = row[0].as_int() as u32;
            let table_name = row[1].as_sym();
            let schema_sym = row[2].as_sym();
            let db_sym = row[3].as_sym();

            let mut cols: Vec<(usize, ColumnEntry)> = attr_rows
                .iter()
                .filter(|r| r[0].as_int() as u32 == table_oid)
                .map(|r| {
                    let idx = r[1].as_int() as usize;
                    let col = ColumnEntry {
                        name: r[2].as_sym(),
                        data_type: parse_data_type(interner.resolve(r[3].as_sym())),
                        nullable: r[4].as_bool(),
                        default: None,
                        is_unique: r[5].as_bool(),
                        is_primary_key: r[6].as_bool(),
                    };
                    (idx, col)
                })
                .collect();
            cols.sort_by_key(|(i, _)| *i);
            let columns = cols.into_iter().map(|(_, c)| c).collect();

            let entry = TableEntry::new(table_oid, table_name, columns, vec![]);

            if let Some(db) = databases.iter_mut().find(|d| d.name == db_sym) {
                if let Some(schema) = db.schemas.get_mut(&schema_sym) {
                    schema.tables.insert(table_name, entry);
                }
            }
        }

        Ok(databases)
    }
}

// ── DataType helpers ─────────────────────────────────────────────────────────

fn format_data_type(dt: &DataType) -> String {
    use DataType::*;
    match dt {
        SmallInt => "SMALLINT".into(),
        Int => "INT".into(),
        BigInt => "BIGINT".into(),
        Boolean => "BOOLEAN".into(),
        Float => "FLOAT".into(),
        Double => "DOUBLE".into(),
        Text => "TEXT".into(),
        VarChar(Some(n)) => format!("VARCHAR({})", n),
        VarChar(None) => "VARCHAR".into(),
        Char(Some(n)) => format!("CHAR({})", n),
        Char(None) => "CHAR".into(),
        Binary => "BINARY".into(),
        VarBinary(Some(n)) => format!("VARBINARY({})", n),
        VarBinary(None) => "VARBINARY".into(),
        Decimal(Some(p), Some(s)) => format!("DECIMAL({},{})", p, s),
        Decimal(Some(p), None) => format!("DECIMAL({})", p),
        Decimal(None, _) => "DECIMAL".into(),
        Json => "JSON".into(),
        JsonB => "JSONB".into(),
        Date => "DATE".into(),
        Time => "TIME".into(),
        Timestamp => "TIMESTAMP".into(),
        Interval => "INTERVAL".into(),
        UUID => "UUID".into(),
        Bytea => "BYTEA".into(),
        Array(inner) => format!("{}[]", format_data_type(inner)),
        Custom(_) => "TEXT".into(), // fallback for custom types
    }
}

fn parse_data_type(s: &str) -> crate::ast::DataType {
    use crate::ast::DataType::*;
    // Parameterised types: VARCHAR(n), CHAR(n), VARBINARY(n), DECIMAL(p,s)
    if s.starts_with("VARCHAR(") {
        let n: u64 = s[8..s.len() - 1].parse().unwrap_or(255);
        return VarChar(Some(n));
    }
    if s.starts_with("CHAR(") {
        let n: u64 = s[5..s.len() - 1].parse().unwrap_or(1);
        return Char(Some(n));
    }
    if s.starts_with("VARBINARY(") {
        let n: u64 = s[10..s.len() - 1].parse().unwrap_or(255);
        return VarBinary(Some(n));
    }
    if s.starts_with("DECIMAL(") {
        let inner = &s[8..s.len() - 1];
        let parts: Vec<&str> = inner.split(',').collect();
        let p = parts[0].parse().ok();
        let sc = parts.get(1).and_then(|v| v.parse().ok());
        return Decimal(p, sc);
    }
    if s.ends_with("[]") {
        let inner = parse_data_type(&s[..s.len() - 2]);
        return Array(Box::new(inner));
    }
    match s {
        "SMALLINT" => SmallInt,
        "INT" => Int,
        "BIGINT" => BigInt,
        "BOOLEAN" => Boolean,
        "FLOAT" => Float,
        "DOUBLE" => Double,
        "TEXT" => Text,
        "VARCHAR" => VarChar(None),
        "CHAR" => Char(None),
        "BINARY" => Binary,
        "VARBINARY" => VarBinary(None),
        "DECIMAL" => Decimal(None, None),
        "JSON" => Json,
        "JSONB" => JsonB,
        "DATE" => Date,
        "TIME" => Time,
        "TIMESTAMP" => Timestamp,
        "INTERVAL" => Interval,
        "UUID" => UUID,
        "BYTEA" => Bytea,
        _ => Text,
    }
}
