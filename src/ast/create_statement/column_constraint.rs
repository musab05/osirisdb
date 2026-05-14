use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnConstraint {
    Null,
    NotNull,

    Default(Expr),

    Unique,

    PrimaryKey,

    Check(Expr),

    References {
        table: Vec<String>,
        columns: Vec<String>,
        on_delete: Option<ReferentialAction>,
        on_update: Option<ReferentialAction>,
    },

    AutoIncrement,
}

