use crate::{
    ast::session::database::UseDatabaseStmt,
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
    /// Parses a `USE` statement.
    ///
    /// Syntax:
    /// ```sql
    /// USE database_name;
    /// ```
    ///
    /// The `USE` keyword is consumed by the caller (`statement.rs`).
    pub fn parse_use_database(&mut self) -> Result<UseDatabaseStmt, ParserError> {
        let database_name = self.expect_identifier()?;
        Ok(UseDatabaseStmt { database_name })
    }
}
