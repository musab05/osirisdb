use crate::ast::*;
#[derive(Debug, Clone, PartialEq)]
pub enum TableConstraint {
    PrimaryKey {
        name: Option<String>,
        columns: Vec<String>,
    },

    Unique {
        name: Option<String>,
        columns: Vec<String>,
    },

    Check {
        name: Option<String>,
        expr: Expr,
    },

    ForeignKey {
        name: Option<String>,
        columns: Vec<String>,
        foreign_table: Vec<String>,
        referred_columns: Vec<String>,

        on_delete: Option<ReferentialAction>,
        on_update: Option<ReferentialAction>,
    },
}

