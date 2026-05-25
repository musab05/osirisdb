use crate::{
    ast::{ObjectName, TruncateStmt},
    lexer::TokenKind,
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
    pub fn parse_truncate(&mut self) -> Result<TruncateStmt, ParserError> {
        self.consume(&TokenKind::Truncate);

        self.consume(&TokenKind::Table);

        let mut tables = vec![];

        loop {
            tables.push(ObjectName(self.parse_qualified_name()?));

            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }

        let restart_identity = if self.consume(&TokenKind::Restart) {
            self.expect(TokenKind::Identity)?;
            true
        } else if self.consume(&TokenKind::Continue) {
            self.expect(TokenKind::Identity)?;
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
