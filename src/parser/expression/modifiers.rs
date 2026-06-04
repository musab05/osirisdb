/// Modifier flags parsed from tokens between the `CREATE` keyword and the object keyword
/// (e.g. `TABLE`, `VIEW`, `INDEX`).
///
/// These flags capture optional SQL modifiers such as `OR REPLACE`, `TEMPORARY`,
/// `UNLOGGED`, `UNIQUE`, and `MATERIALIZED` that can appear in various combinations
/// depending on the DDL statement being parsed.
pub struct CreateModifiers {
    /// `OR REPLACE` — if `true`, the object should be replaced if it already exists.
    pub or_replace: bool,
    /// `TEMPORARY` / `TEMP` — if `true`, the object is session-scoped.
    pub temporary: bool,
    /// `UNLOGGED` — if `true`, the table is not written to the write-ahead log (PostgreSQL).
    pub unlogged: bool,
    /// `UNIQUE` — if `true`, applies a uniqueness constraint (used for indexes).
    pub unique: bool,
    /// `MATERIALIZED` — if `true`, the view stores query results physically.
    pub materialized: bool,
}