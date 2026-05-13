#[derive(Debug, Clone, PartialEq)]
pub enum ConflictAction {
    DoNothing,
    DoUpdate {
        assignments: Vec<Assignment>,
        where_: Option<Expr>,
    },
}
