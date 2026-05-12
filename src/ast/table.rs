pub enum TableRef {
    Named {
        name: Vec<String>,
        alias: Option<String>,
    },

    Subquery {
        query: Box<SelectStmt>,
        alias: Option<String>,
    },
}
