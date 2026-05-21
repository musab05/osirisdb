use crate::{
    ast::{ObjectName, Statement, TruncateStmt},
    lexer::Token,
    parser::{parser::Parser, parser_error::ParserError},
};

impl Parser {
    pub fn parse_truncate(&mut self) -> Result<TruncateStmt, ParserError> {
        self.consume(&Token::Truncate);

        self.consume(&Token::Table);

        let mut tables = vec![];

        loop {
            tables.push(ObjectName(self.parse_qualified_name()?));

            if !self.consume(&Token::Comma) {
                break;
            }
        }

        let restart_identity = if self.consume(&Token::Restart) {
            self.expect(Token::Identity)?;
            true
        } else if self.consume(&Token::Continue) {
            self.expect(Token::Identity)?;
            false
        } else {
            false
        };

        let behaviour = self.parse_drop_behaviour();

        Ok(TruncateStmt {
            tables,
            restart_identity,
            behaviour,
        })
    }
}
