use crate::{ast::CreateSequenceStmt, lexer::TokenKind, parser::{parser::Parser, parser_error::ParserError}};

impl<'a> Parser<'a> {
    pub fn parse_create_sequence(&mut self) -> Result<CreateSequenceStmt, ParserError> {
        self.expect(TokenKind::Sequence)?;
        todo!()
    }
}