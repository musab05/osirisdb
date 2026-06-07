use crate::{ast::*, common::symbol::Symbol};

/// Represents a full `CREATE TRIGGER` statement.
///
/// Compatible with PostgreSQL trigger syntax while adding:
/// - `TAGS` — metadata labels for categorizing triggers (our extension)
/// - `PRIORITY` — execution order when multiple triggers fire on same event (our extension)
/// - `ENABLED` — initial enabled/disabled state at creation time (our extension)
/// - Transition table referencing (PostgreSQL 10+)
/// - Constraint triggers with deferral support (PostgreSQL compatible)
#[derive(Debug, Clone, PartialEq)]
pub struct CreateTriggerStmt {
    pub name: Symbol,
    pub if_not_exists: bool,
    pub or_replace: bool,

    /// CONSTRAINT — marks this as a constraint trigger.
    /// Constraint triggers support DEFERRABLE and INITIALLY clauses.
    pub constraint: bool,

    /// BEFORE | AFTER | INSTEAD OF
    pub timing: TriggerTiming,

    /// One or more of INSERT | UPDATE | DELETE | TRUNCATE
    /// separated by OR: `INSERT OR UPDATE OR DELETE`
    pub events: Vec<TriggerEvent>,

    /// Table the trigger is attached to
    pub table: ObjectName,

    /// FROM referenced_table — only valid for constraint triggers.
    /// References the table that the foreign key constraint references.
    pub from_table: Option<ObjectName>,

    /// DEFERRABLE | NOT DEFERRABLE — only valid for constraint triggers.
    /// Controls whether the trigger can be deferred to end of transaction.
    pub deferrable: Option<bool>,

    /// INITIALLY DEFERRED | INITIALLY IMMEDIATE — only for constraint triggers.
    /// Sets the default deferral state within a transaction.
    pub initially: Option<TriggerInitially>,

    /// REFERENCING OLD TABLE AS x NEW TABLE AS y
    /// Allows access to transition tables (full set of affected rows).
    /// Only valid for AFTER triggers on INSERT, UPDATE, DELETE.
    pub referencing: Vec<TriggerReferencing>,

    /// FOR EACH ROW | FOR EACH STATEMENT
    pub level: TriggerLevel,

    /// Optional WHEN (condition) — trigger only fires when this is true.
    /// In row-level triggers, OLD and NEW refer to the affected row.
    pub condition: Option<Expr>,

    /// EXECUTE FUNCTION/PROCEDURE name(args)
    pub function: TriggerFunction,

    // ── Our extensions ──
    /// PRIORITY n — integer priority for ordering trigger execution
    /// when multiple triggers fire on the same event and table.
    /// Lower number = fires first. Default is 0 if not specified.
    /// Not present in any standard SQL or PostgreSQL — our addition.
    ///
    /// Example:
    /// ```sql
    /// CREATE TRIGGER audit_log
    ///     AFTER INSERT ON orders
    ///     FOR EACH ROW
    ///     PRIORITY 10
    ///     EXECUTE FUNCTION log_order();
    /// ```
    pub priority: Option<i64>,

    /// TAGS ('tag1', 'tag2') — arbitrary string labels for the trigger.
    /// Useful for documentation, tooling, and selective enable/disable.
    /// Not present in any standard SQL or PostgreSQL — our addition.
    ///
    /// Example:
    /// ```sql
    /// CREATE TRIGGER audit_log
    ///     AFTER INSERT ON orders
    ///     FOR EACH ROW
    ///     TAGS ('audit', 'compliance')
    ///     EXECUTE FUNCTION log_order();
    /// ```
    pub tags: Vec<Symbol>,

    /// ENABLED | DISABLED — sets the initial state of the trigger.
    /// PostgreSQL has `ALTER TRIGGER ... ENABLE/DISABLE` but not at creation.
    /// We allow it at `CREATE` time to avoid a separate ALTER statement.
    ///
    /// Example:
    /// ```sql
    /// CREATE TRIGGER audit_log
    ///     AFTER INSERT ON orders
    ///     FOR EACH ROW
    ///     DISABLED
    ///     EXECUTE FUNCTION log_order();
    /// ```
    pub enabled: bool,
}

/// When the trigger fires relative to the triggering event.
///
/// - `Before` — fires before the row is modified
/// - `After` — fires after the row is modified and constraints are checked
/// - `InsteadOf` — replaces the triggering operation, only valid on views
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerTiming {
    Before,
    After,
    InsteadOf,
}

/// The DML event that activates the trigger.
///
/// `Update` carries an optional column list — when specified the trigger
/// only fires if one of those columns is modified.
///
/// Example: `UPDATE OF price, stock`
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerEvent {
    Insert,
    Update(Vec<Symbol>), // empty Vec = all columns
    Delete,
    Truncate,
}

/// Granularity at which the trigger fires.
///
/// - `Row` — fires once per affected row
/// - `Statement` — fires once per SQL statement regardless of row count
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerLevel {
    Row,
    Statement,
}

/// The function invoked when the trigger fires.
///
/// `args` are string literal arguments passed to the function.
/// PostgreSQL only allows string literals here, not expressions.
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerFunction {
    /// Qualified function name e.g. `ObjectName`
    pub name: ObjectName,
    /// String literal arguments: `EXECUTE FUNCTION my_func('arg1', 'arg2')`
    pub args: Vec<Symbol>,
}

/// A single entry in the `REFERENCING` clause.
///
/// Gives a name to the transition table (full set of old or new rows)
/// so the trigger function can query it as a regular table.
///
/// Example:
/// ```sql
/// REFERENCING OLD TABLE AS deleted_rows NEW TABLE AS inserted_rows
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerReferencing {
    pub kind: TransitionKind,
    /// The alias used to reference this transition table in the function
    pub alias: Symbol,
}

/// Which transition table is being named in a REFERENCING clause.
#[derive(Debug, Clone, PartialEq)]
pub enum TransitionKind {
    /// OLD TABLE — contains rows as they were before the operation
    OldTable,
    /// NEW TABLE — contains rows as they will be after the operation
    NewTable,
}

/// Deferral timing for constraint triggers.
#[derive(Debug, Clone, PartialEq)]
pub enum TriggerInitially {
    /// Check constraint at end of transaction
    Deferred,
    /// Check constraint immediately after each statement (default)
    Immediate,
}
