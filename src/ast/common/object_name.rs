/// Represents a qualified SQL object name (e.g. `table_name`, `schema_name.table_name`, or `db.schema.table`).
///
/// Contains a sequence of identifiers representing the parts of the qualified path.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectName(pub Vec<String>);
