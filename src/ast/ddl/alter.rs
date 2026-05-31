use crate::ast::*;

/// Represents a DDL statement to modify an existing table schema or properties (`ALTER TABLE`).
#[derive(Debug, Clone, PartialEq)]
pub struct AlterTableStmt {
    /// Skip error creation if the table does not exist (`IF EXISTS`).
    pub if_exist: bool,
    /// The qualified name of the table to alter.
    pub name: ObjectName,
    /// The list of actions to perform on the table.
    pub actions: Vec<AlterTableAction>,
}

/// The specific action to apply to the table in an `ALTER TABLE` statement.
#[derive(Debug, Clone, PartialEq)]
pub enum AlterTableAction {
    /// Add a new column to the table (`ADD [COLUMN] [IF NOT EXISTS] column_def`).
    AddColumn {
        /// If `true`, does nothing if the column already exists.
        if_not_exist: bool,
        /// The new column definition.
        column: ColumnDef,
    },
    /// Drop a column from the table (`DROP [COLUMN] [IF EXISTS] name [CASCADE|RESTRICT]`).
    DropColumn {
        /// If `true`, does nothing if the column does not exist.
        if_exist: bool,
        /// The column name.
        name: String,
        /// Optional cascade/restrict behavior for dropping associated dependencies.
        behaviour: Option<DropBehavior>,
    },
    /// Modify an existing column's definition (e.g. data type, default value).
    AlterColumn {
        /// The name of the column to alter.
        name: String,
        /// The modification action to perform on this column.
        action: AlterColumnAction,
    },
    /// Rename a column (`RENAME COLUMN old_name TO new_name`).
    RenameColumn {
        /// The current name of the column.
        old_name: String,
        /// The new name for the column.
        new_name: String,
    },

    /// Add a new table-level constraint (`ADD CONSTRAINT constraint_def`).
    AddConstraint(TableConstraint),
    /// Drop a table-level constraint (`DROP CONSTRAINT [IF EXISTS] name [CASCADE|RESTRICT]`).
    DropConstraint {
        /// If `true`, does nothing if the constraint does not exist.
        if_exist: bool,
        /// The constraint name.
        name: String,
        /// Optional cascade/restrict behavior for dropping dependencies.
        behaviour: Option<DropBehavior>,
    },
    /// Rename a constraint (`RENAME CONSTRAINT old_name TO new_name`).
    RenameConstraint {
        /// The current constraint name.
        old_name: String,
        /// The new name for the constraint.
        new_name: String,
    },

    /// Rename the table (`RENAME TO new_name`).
    RenameTable(String),
    /// Change the table's schema (`SET SCHEMA new_schema`).
    SetSchema(String),
    /// Change the table owner (`OWNER TO new_owner`).
    SetOwner(String),
    /// Change the table's tablespace (`SET TABLESPACE new_tablespace`).
    SetTableSpace(String),
    /// Set specific table parameters (`SET (options)`).
    SetOptions(Vec<SqlOption>),
    /// Reset/Remove specific table parameters (`RESET (options)`).
    ResetOptions(Vec<String>),

    /// Add a parent table to inherit from (`INHERIT parent_table`).
    Inherit(ObjectName),
    /// Remove inheritance from a parent table (`NO INHERIT parent_table`).
    NoInherit(ObjectName),

    /// Attach a partition table (`ATTACH PARTITION partition_name FOR VALUES ...`).
    AttachPartition {
        /// The name of the partition table.
        partition: ObjectName,
        /// The partition boundary specification.
        for_values: PartitionBound,
    },
    /// Detach a partition table (`DETACH PARTITION partition_name`).
    DetachPartition(ObjectName),
}

/// The specific action to apply to a column in an `ALTER COLUMN` statement.
#[derive(Debug, Clone, PartialEq)]
pub enum AlterColumnAction {
    /// Change the column's data type, optionally changing collation or using a conversion expression (`SET DATA TYPE type [COLLATE c] [USING expr]`).
    SetType {
        /// The new data type.
        data_type: DataType,
        /// Optional custom collation name.
        collation: Option<String>,
        /// Optional expression to convert current data to the new type.
        using: Option<Expr>,
    },
    /// Set a new default value for the column (`SET DEFAULT expr`).
    SetDefault(Expr),
    /// Remove the column's default value (`DROP DEFAULT`).
    DropDefault,
    /// Enforce that the column cannot store nulls (`SET NOT NULL`).
    SetNotNull,
    /// Allow the column to store nulls (`DROP NOT NULL`).
    DropNotNull,
    /// Set statistical logging target (`SET STATISTICS integer`).
    SetStatistics(i64),
    /// Set column-specific options.
    SetOptions(Vec<SqlOption>),
    /// Reset/Remove column-specific options.
    ResetOptions(Vec<String>),
    /// Change the storage mode of the column (e.g. `PLAIN`, `MAIN`).
    SetStorage(ColumnStorage),
}

/// PostgreSQL column storage options.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnStorage {
    /// Plain values stored inline (no compression or out-of-line storage).
    Plain,
    /// Compression is allowed, inline storage only.
    External,
    /// Compression and out-of-line storage are allowed (default for most compressible types).
    Extended,
    /// Compression is allowed, but out-of-line storage is used only as a last resort.
    Main,
}

/// Represents the partition bound values for list, range, or hash partitioning.
#[derive(Debug, Clone, PartialEq)]
pub enum PartitionBound {
    /// Boundary for list partitioning (`IN (value1, ...)`).
    In(Vec<Expr>),
    /// Boundaries for range partitioning (`FROM (low, ...) TO (high, ...)`).
    FromTo {
        /// The lower boundary list.
        from: Vec<PartitionBoundValue>,
        /// The upper boundary list.
        to: Vec<PartitionBoundValue>,
    },
    /// Boundary for hash partitioning (`WITH (MODULUS m, REMAINDER r)`).
    With(Vec<Expr>),
    /// Default partition catch-all for list/range partitioning (`DEFAULT`).
    Default,
}

/// A boundary value limit in a range partition definition (e.g. `MINVALUE` or a expression).
#[derive(Debug, Clone, PartialEq)]
pub enum PartitionBoundValue {
    /// Evaluated scalar expression boundary value.
    Expr(Expr),
    /// Minimum possible value representation (`MINVALUE`).
    Minvalue,
    /// Maximum possible value representation (`MAXVALUE`).
    Maxvalue,
}
