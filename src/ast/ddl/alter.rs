use crate::ast::*;

#[derive(Debug, Clone, PartialEq)]
pub struct AlterTableStmt {
    pub if_exist: bool,
    pub name: ObjectName,
    pub actions: Vec<AlterTableAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterTableAction {
    // Columns
    AddColumn {
        if_not_exist: bool,
        column: ColumnDef,
    },
    DropColumn {
        if_exist: bool,
        name: String,
        behaviour: Option<DropBehavior>,
    },
    AlterColumn {
        name: String,
        action: AlterColumnAction,
    },
    RenameColumn {
        old_name: String,
        new_name: String,
    },

    // Constraints
    AddConstraint(TableConstraint),
    DropConstraint {
        if_exist: bool,
        name: String,
        behaviour: Option<DropBehavior>,
    },
    RenameConstraint {
        old_name: String,
        new_name: String,
    },

    // Table-level
    RenameTable(String),
    SetSchema(String),
    SetOwner(String),
    SetTableSpace(String),
    SetOptions(Vec<SqlOption>),
    ResetOptions(Vec<String>),

    // Inheritance
    Inherit(ObjectName),
    NoInherit(ObjectName),

    // Partitioning
    AttachPartition {
        partition: ObjectName,
        for_values: PartitionBound,
    },
    DetachPartition(ObjectName),
}

#[derive(Debug, Clone, PartialEq)]
pub enum AlterColumnAction {
    SetType {
        data_type: DataType,
        collation: Option<String>,
        using: Option<Expr>,
    },
    SetDefault(Expr),
    DropDefault,
    SetNotNull,
    DropNotNull,
    SetStatistics(i64),
    SetOptions(Vec<SqlOption>),
    ResetOptions(Vec<String>),
    SetStorage(ColumnStorage),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ColumnStorage {
    Plain,
    External,
    Extended,
    Main,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PartitionBound {
    In(Vec<Expr>),
    FromTo {
        from: Vec<PartitionBoundValue>,
        to: Vec<PartitionBoundValue>,
    },
    With(Vec<Expr>),
    Default,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PartitionBoundValue {
    Expr(Expr),
    Minvalue,
    Maxvalue,
}
