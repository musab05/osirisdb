use crate::{
    ast::{CreateViewStmt, ViewCheckOption},
    lexer::TokenKind,
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
    pub fn parse_create_view(
        &mut self,
        or_replace: bool,
        temporary: bool,
        recursive: bool,
    ) -> Result<CreateViewStmt, ParserError> {
        let name = self.parse_qualified_name()?;
        let columns = if self.consume(&TokenKind::LParen) {
            let mut cols = vec![];

            loop {
                cols.push(self.expect_identifier()?);
                if !self.consume(&TokenKind::Comma) {
                    break;
                }
            }
            self.expect(TokenKind::RParen)?;
            cols
        } else {
            vec![]
        };

        let with_options = if self.consume(&TokenKind::With) {
            self.parse_options_list()?
        } else {
            vec![]
        };

        self.expect(TokenKind::As)?;

        let query = Box::new(self.parse_select()?);

        let check_option = if self.consume(&TokenKind::With) {
            if matches!(self.current_token(), TokenKind::Ident) {
                let word =
                    self.source[self.current.span.start..self.current.span.end].to_uppercase();

                match word.as_str() {
                    "LOCAL" => {
                        self.advance();
                        self.expect_keyword_sequence(&[TokenKind::Check, TokenKind::Options])?;
                        Some(ViewCheckOption::Local)
                    }
                    "CASCADE" => {
                        self.advance();
                        self.expect_keyword_sequence(&[TokenKind::Check, TokenKind::Options])?;
                        Some(ViewCheckOption::Cascaded)
                    }
                    _ => {
                        self.expect_keyword_sequence(&[TokenKind::Check, TokenKind::Options])?;
                        Some(ViewCheckOption::Cascaded) // default
                    }
                }
            } else {
                self.expect_keyword_sequence(&[TokenKind::Check, TokenKind::Options])?;
                Some(ViewCheckOption::Cascaded)
            }
        } else {
            None
        };

        Ok(CreateViewStmt {
            or_replace,
            temporary,
            recursive,
            name,
            columns,
            with_options,
            query,
            check_option,
        })
    }
}
