use crate::{
    ast::{
        ColumnConstraint, ColumnDef, CreateStmt, GeneratedColumn, OnCommit, PartitionClause,
        PartitionKind, ReferentialAction, SqlOption, Statement, TableConstraint,
    },
    lexer::token::Modifier,
    lexer::token::Token,
    parser::{parser::Parser, parser_error::ParserError},
};

impl Parser {
    pub fn parse_create(&mut self) -> Result<Statement, ParserError> {
        self.consume(&Token::Create);

        let m = self.parse_create_modifiers();

        
        match self.current_token() {
            Token::Table => {
                let stmt = self.parse_create_table(m.temporary, m.unlogged)?;
                Ok(Statement::CreateTable(stmt))
            }
            _ => Err(ParserError::new(
                format!(
                    "Expected TABLE after CREATE, got {:?}",
                    self.current_token()
                ),
                self.current.span.clone(),
            )),
        }
    }
}
