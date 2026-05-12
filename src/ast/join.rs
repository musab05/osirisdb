pub struct JoinClause {
    pub join_type: JoinType,
    pub table: TableRef,
    pub condition: Option<Expr>,
}

pub enum JoinType {
    Inner,
    Left,
    Right,
    Full,
    Cross,
}
