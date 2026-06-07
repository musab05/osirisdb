use crate::{
    ast::{CreateSequenceStmt, ObjectName},
    lexer::TokenKind,
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
    /// Executes parsing or lookup for the `parse_create_sequence` operation.
    pub fn parse_create_sequence(&mut self) -> Result<CreateSequenceStmt, ParserError> {
        let if_not_exists = self.parse_if_not_exist()?;
        let name = ObjectName(self.parse_qualified_name()?);

        let mut data_type = None;
        let mut start = None;
        let mut increment = None;
        let mut minvalue = None;
        let mut maxvalue = None;
        let mut cache = None;
        let mut cycle = None;
        let mut owned_by = None;

        loop {
            match self.current_token().clone() {
                // AS integer/bigint/smallint
                TokenKind::As => {
                    self.advance();
                    data_type = Some(self.parse_data_type()?);
                }

                // START WITH n or START n
                TokenKind::Start => {
                    self.advance();
                    self.consume(&TokenKind::With);
                    start = Some(self.expect_int()?);
                }

                // INCREMENT BY n
                TokenKind::Increment => {
                    self.advance();
                    self.expect(TokenKind::By)?;
                    increment = Some(self.expect_int()?);
                }

                // MINVALUE n or NO MINVALUE
                TokenKind::Minvalue => {
                    self.advance();
                    minvalue = Some(self.expect_int()?);
                }

                // MAXVALUE n or NO MAXVALUE
                TokenKind::Maxvalue => {
                    self.advance();
                    maxvalue = Some(self.expect_int()?);
                }

                // NO MINVALUE / NO MAXVALUE / NO CYCLE
                TokenKind::No => {
                    self.advance();
                    match self.current_token().clone() {
                        TokenKind::Minvalue => {
                            self.advance();
                            minvalue = None;
                        }
                        TokenKind::Maxvalue => {
                            self.advance();
                            maxvalue = None;
                        }
                        TokenKind::Cycle => {
                            self.advance();
                            cycle = Some(false);
                        }
                        _ => {
                            return Err(ParserError::new(
                                "Expected MINVALUE, MAXVALUE or CYCLE after NO",
                                self.current.span.clone(),
                            ));
                        }
                    }
                }

                // CACHE n
                TokenKind::Cache => {
                    self.advance();
                    cache = Some(self.expect_int()?);
                }

                // CYCLE
                TokenKind::Cycle => {
                    self.advance();
                    cycle = Some(true);
                }

                // OWNED BY table.col or OWNED BY NONE
                TokenKind::Owned => {
                    self.advance();
                    self.expect(TokenKind::By)?;
                    if matches!(self.current_token(), TokenKind::Ident)
                        && self.source[self.current.span.start..self.current.span.end]
                            .to_uppercase()
                            == "NONE"
                    {
                        self.advance();
                        owned_by = None;
                    } else {
                        let mut parts = vec![self.expect_identifier()?];
                        while self.consume(&TokenKind::Dot) {
                            parts.push(self.expect_identifier()?);
                        }
                        owned_by = Some(ObjectName(parts));
                    }
                }

                _ => break,
            }
        }

        Ok(CreateSequenceStmt {
            name,
            if_not_exists,
            data_type,
            start,
            increment,
            minvalue,
            maxvalue,
            cache,
            cycle,
            owned_by,
        })
    }
}
