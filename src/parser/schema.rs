use crate::{
    ast::CreateSchemaStmt,
    lexer::TokenKind,
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
    pub fn parse_create_schema(&mut self) -> Result<CreateSchemaStmt, ParserError> {

        let if_not_exists = self.parse_if_not_exist()?;

        let name = if matches!(self.current_token(), TokenKind::Ident | TokenKind::QuotedIdent) {
            Some(self.expect_identifier()?)
        } else {
            None
        };

        let authorization = if self.consume(&TokenKind::Authorization) {
            Some(self.expect_identifier()?)
        } else {
            None
        };

        if name.is_none() && authorization.is_none() {
            return Err(ParserError::new(
                "CREATE SCHEMA requires a name or AUTHORIZATION",
                self.current.span.clone(),
            ));
        }

        Ok(CreateSchemaStmt {
            name,
            authorization,
            if_not_exists,
        })
    }
}