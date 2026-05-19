use crate::{
    ast::{
        ColumnConstraint, ColumnDef, CreateStmt, GeneratedColumn, OnCommit, PartitionClause,
        PartitionKind, ReferentialAction, SqlOption, TableConstraint,
    },
    lexer::token::Token,
    parser::{parser::Parser, parser_error::ParserError},
};

impl Parser {
    pub fn parse_create(&mut self) -> Result<CreateStmt, ParserError> {
        self.consume(&Token::Create);

        let mut temporary = false;
        let mut unlogged = false;

        match self.current_token() {
            Token::Ident(k)
                if k.eq_ignore_ascii_case("temporary") || k.eq_ignore_ascii_case("temp") =>
            {
                temporary = true;
                self.advance();
            }
            Token::Ident(k) if k.eq_ignore_ascii_case("unlogged") => {
                unlogged = true;
                self.advance();
            }

            _ => {}
        }

        self.expect(Token::Table)?;

        let if_not_exist = self.parse_if_not_exist()?;

        let name = self.parse_qualified_name()?;

        self.expect(Token::LParen)?;

        let mut columns = vec![];
        let mut constraints = vec![];

        loop {
            if *self.current_token() == Token::RParen {
                break;
            }

            if self.is_table_constraint() {
                constraints.push(self.parse_table_constraint()?);
            } else {
                columns.push(self.parse_column_def()?);
            }

            if !self.consume(&Token::Comma) {
                break;
            }
        }
        self.expect(Token::RParen)?;

        let mut inherits = vec![];
        let mut partitions = vec![];
        let mut with_options = vec![];
        let mut table_space = None;
        let mut on_commit = None;
        let mut as_query = None;

        loop {
            match self.current_token().clone() {
                Token::Ident(k) if k.eq_ignore_ascii_case("inherits") => {
                    self.advance();
                    self.expect(Token::LParen)?;
                    loop {
                        inherits.push(self.parse_qualified_name()?);
                        if !self.consume(&Token::Comma) {
                            break;
                        }
                    }
                    self.expect(Token::RParen)?;
                }
                Token::Ident(k) if k.eq_ignore_ascii_case("partition") => {
                    self.advance();
                    self.expect(Token::By)?;

                    let kind = match self.current_token() {
                        Token::Ident(k) if k.eq_ignore_ascii_case("range") => PartitionKind::Range,
                        Token::Ident(k) if k.eq_ignore_ascii_case("list") => PartitionKind::List,
                        Token::Ident(k) if k.eq_ignore_ascii_case("hash") => PartitionKind::Hash,
                        _ => {
                            return Err(ParserError::new(
                                "Expected RANGE, LIST, or HASH after PARTITION BY",
                                self.current.span.clone(),
                            ));
                        }
                    };
                    self.advance();

                    self.expect(Token::LParen)?;
                    let mut exprs = vec![];
                    loop {
                        exprs.push(self.parse_expr()?);
                        if !self.consume(&Token::Comma) {
                            break;
                        }
                    }
                    self.expect(Token::RParen)?;

                    partitions.push(PartitionClause { kind, exprs });
                }
                Token::With => {
                    self.advance();
                    self.expect(Token::LParen)?;
                    loop {
                        let name = self.expect_identifier()?;
                        self.expect(Token::Eq)?;
                        let value = self.parse_expr()?;
                        with_options.push(SqlOption { name, value });
                        if !self.consume(&Token::Comma) {
                            break;
                        }
                    }
                    self.expect(Token::RParen)?;
                }
                Token::Ident(k) if k.eq_ignore_ascii_case("tablespace") => {
                    self.advance();
                    table_space = Some(self.expect_identifier()?);
                }
                Token::On => {
                    self.advance();
                    if matches!(self.current_token(), Token::Commit)
                        || matches!(self.current_token(), Token::Ident(i) if i.eq_ignore_ascii_case("commit"))
                    {
                        self.advance();
                        match self.current_token() {
                            Token::Ident(i) if i.eq_ignore_ascii_case("preserve") => {
                                self.advance();
                                if matches!(self.current_token(), Token::Ident(r) if r.eq_ignore_ascii_case("rows"))
                                {
                                    self.advance();
                                }
                                on_commit = Some(OnCommit::PreserveRows);
                            }
                            Token::Ident(i) if i.eq_ignore_ascii_case("delete") => {
                                self.advance();
                                if matches!(self.current_token(), Token::Ident(r) if r.eq_ignore_ascii_case("rows"))
                                {
                                    self.advance();
                                }
                                on_commit = Some(OnCommit::DeleteRows);
                            }
                            Token::Ident(i) if i.eq_ignore_ascii_case("drop") => {
                                self.advance();
                                on_commit = Some(OnCommit::Drop);
                            }
                            _ => {
                                return Err(ParserError::new(
                                    "Expected PRESERVE ROWS, DELETE ROWS, or DROP after ON COMMIT",
                                    self.current.span.clone(),
                                ));
                            }
                        }
                    } else {
                        return Err(ParserError::new(
                            "Expected COMMIT after ON",
                            self.current.span.clone(),
                        ));
                    }
                }
                Token::As => {
                    self.advance();
                    as_query = Some(self.parse_select()?);
                }
                _ => break,
            }
        }

        Ok(CreateStmt {
            if_not_exist,
            temporary,
            unlogged,
            name,
            columns,
            constraints,
            inherits,
            partitions,
            with_options,
            table_space,
            on_commit,
            as_query,
        })
    }

    fn parse_if_not_exist(&mut self) -> Result<bool, ParserError> {
        if matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("if")) {
            self.advance();
            self.expect(Token::Not)?;
            self.expect(Token::Exists)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn parse_qualified_name(&mut self) -> Result<Vec<String>, ParserError> {
        let mut parts = vec![self.expect_identifier()?];

        while self.consume(&Token::Dot) {
            parts.push(self.expect_identifier()?);
        }
        Ok(parts)
    }

    fn is_table_constraint(&self) -> bool {
        matches!(
            self.current_token(),
            Token::Primary | Token::Foreign | Token::Unique | Token::Check
        ) || matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("constraint"))
    }

    fn parse_column_def(&mut self) -> Result<ColumnDef, ParserError> {
        let name = self.expect_identifier()?;
        let data_type = self.parse_data_type()?;

        let collation = if matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("collate"))
        {
            self.advance();
            Some(self.expect_identifier()?)
        } else {
            None
        };

        let mut constraints = vec![];
        let mut generated = None;

        loop {
            match self.current_token().clone() {
                Token::Ident(ref k) if k.eq_ignore_ascii_case("constraint") => {
                    self.advance();
                    let _constraint_name = self.expect_identifier()?;
                    constraints.push(self.parse_column_constraint()?);
                }
                Token::Ident(ref k) if k.eq_ignore_ascii_case("generated") => {
                    generated = Some(self.parse_generated_column()?);
                }
                Token::Ident(ref k)
                    if k.eq_ignore_ascii_case("autoincrement")
                        || k.eq_ignore_ascii_case("auto_increment") =>
                {
                    self.advance();
                    constraints.push(ColumnConstraint::AutoIncrement);
                }

                Token::Not
                | Token::Null
                | Token::Default
                | Token::Unique
                | Token::Primary
                | Token::References
                | Token::Check => {
                    constraints.push(self.parse_column_constraint()?);
                }
                _ => break,
            }
        }

        Ok(ColumnDef {
            name,
            data_type,
            collation,
            constraints,
            generated,
        })
    }

    fn parse_column_constraint(&mut self) -> Result<ColumnConstraint, ParserError> {
        match self.current_token().clone() {
            Token::Not => {
                self.advance();
                self.expect(Token::Null)?;
                Ok(ColumnConstraint::NotNull)
            }
            Token::Null => {
                self.advance();
                Ok(ColumnConstraint::Null)
            }
            Token::Default => {
                self.advance();
                Ok(ColumnConstraint::Default(self.parse_expr()?))
            }
            Token::Unique => {
                self.advance();
                Ok(ColumnConstraint::Unique)
            }
            Token::Primary => {
                self.advance();
                self.expect(Token::Key)?;
                Ok(ColumnConstraint::PrimaryKey)
            }
            Token::Check => {
                self.advance();
                self.expect(Token::LParen)?;
                let expr = self.parse_expr()?;
                self.expect(Token::RParen)?;

                Ok(ColumnConstraint::Check(expr))
            }
            Token::References => {
                self.advance();
                let table = self.parse_qualified_name()?;
                let columns = if *self.current_token() == Token::LParen {
                    self.parse_column_list()?
                } else {
                    vec![]
                };

                let (on_delete, on_update) = self.parse_referential_actions()?;
                Ok(ColumnConstraint::References {
                    table,
                    columns,
                    on_delete,
                    on_update,
                })
            }

            _ => Err(ParserError::new(
                format!("Unknown column constraint: {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }

    fn parse_generated_column(&mut self) -> Result<GeneratedColumn, ParserError> {
        self.advance();

        if !matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("always")) {
            return Err(ParserError::new(
                "Expected ALWAYS after GENERATED",
                self.current.span.clone(),
            ));
        }

        self.advance();

        self.expect(Token::As)?;
        self.expect(Token::LParen)?;
        let expr = self.parse_expr()?;
        self.expect(Token::RParen)?;

        if matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("stored")) {
            self.advance();
        }

        Ok(GeneratedColumn { expr, stored: true })
    }

    fn parse_table_constraint(&mut self) -> Result<TableConstraint, ParserError> {
        let name = if matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("constraint"))
        {
            self.advance();
            Some(self.expect_identifier()?)
        } else {
            None
        };

        match self.current_token().clone() {
            Token::Primary => {
                self.advance();
                self.expect(Token::Key)?;
                let columns = self.parse_column_list()?;
                Ok(TableConstraint::PrimaryKey { name, columns })
            }
            Token::Unique => {
                self.advance();
                let columns = self.parse_column_list()?;
                Ok(TableConstraint::Unique { name, columns })
            }
            Token::Check => {
                self.advance();
                self.expect(Token::LParen)?;
                let expr = self.parse_expr()?;
                self.expect(Token::RParen)?;
                Ok(TableConstraint::Check { name, expr })
            }
            Token::Foreign => {
                self.advance();
                self.expect(Token::Key)?;
                let columns = self.parse_column_list()?;
                self.expect(Token::References)?;
                let foreign_table = self.parse_qualified_name()?;
                let referred_columns = if *self.current_token() == Token::LParen {
                    self.parse_column_list()?
                } else {
                    vec![]
                };

                let (on_delete, on_update) = self.parse_referential_actions()?;
                Ok(TableConstraint::ForeignKey {
                    name,
                    columns,
                    foreign_table,
                    referred_columns,
                    on_delete,
                    on_update,
                })
            }

            _ => Err(ParserError::new(
                format!(
                    "Expected PRIMARY, UNIQUE, CHECK, or FOREIGN after CONSTRAINT, found {:?}",
                    self.current_token()
                ),
                self.current.span.clone(),
            )),
        }
    }

    fn parse_decimal_args(&mut self) -> Result<(Option<u8>, Option<u8>), ParserError> {
        if !self.consume(&Token::LParen) {
            return Ok((None, None));
        }

        let prec = self.expect_int_literal()? as u8;
        let scale = if self.consume(&Token::Comma) {
            Some(self.expect_int_literal()? as u8)
        } else {
            None
        };

        self.expect(Token::RParen)?;
        Ok((Some(prec), scale))
    }

    fn skip_time_zone(&mut self) {
        if matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("with") || k.eq_ignore_ascii_case("without"))
        {
            self.advance();
            if matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("time")) {
                self.advance();
                if matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("zone"))
                {
                    self.advance();
                }
            }
        }
    }

    fn parse_column_list(&mut self) -> Result<Vec<String>, ParserError> {
        self.expect(Token::LParen)?;
        let mut cols = vec![self.expect_identifier()?];

        while self.consume(&Token::Comma) {
            cols.push(self.expect_identifier()?);
        }

        self.expect(Token::RParen)?;
        Ok(cols)
    }

    fn parse_referential_actions(
        &mut self,
    ) -> Result<(Option<ReferentialAction>, Option<ReferentialAction>), ParserError> {
        let mut on_delete = None;
        let mut on_update = None;

        for _ in 0..2 {
            if !matches!(self.current_token(), Token::On) {
                break;
            }

            self.advance();

            match self.current_token().clone() {
                Token::Delete => {
                    self.advance();
                    on_delete = Some(self.parse_referential_action()?);
                }
                Token::Ident(ref kw) if kw.eq_ignore_ascii_case("delete") => {
                    self.advance();
                    on_delete = Some(self.parse_referential_action()?);
                }
                Token::Update => {
                    self.advance();
                    on_update = Some(self.parse_referential_action()?);
                }
                Token::Ident(ref kw) if kw.eq_ignore_ascii_case("update") => {
                    self.advance();
                    on_update = Some(self.parse_referential_action()?);
                }

                _ => {
                    return Err(ParserError::new(
                        "Expected DELETE or UPDATE after ON",
                        self.current.span.clone(),
                    ));
                }
            }
        }

        Ok((on_delete, on_update))
    }

    fn parse_referential_action(&mut self) -> Result<ReferentialAction, ParserError> {
        match self.current_token().clone() {
            Token::Set => {
                self.advance(); // SET
                match self.current_token() {
                    Token::Null => {
                        self.advance();
                        Ok(ReferentialAction::SetNull)
                    }
                    Token::Default => {
                        self.advance();
                        Ok(ReferentialAction::SetDefault)
                    }
                    _ => Err(ParserError::new(
                        "Expected NULL or DEFAULT after SET",
                        self.current.span.clone(),
                    )),
                }
            }
            Token::Ident(ref k) => {
                match k.to_uppercase().as_str() {
                    "CASCADE" => {
                        self.advance();
                        Ok(ReferentialAction::Cascade)
                    }
                    "RESTRICT" => {
                        self.advance();
                        Ok(ReferentialAction::Restrict)
                    }
                    "NO" => {
                        self.advance(); // NO
                        if matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("action"))
                        {
                            self.advance(); // ACTION
                        }
                        Ok(ReferentialAction::NoAction)
                    }
                    _ => Err(ParserError::new(
                        format!("Unknown referential action: {k}"),
                        self.current.span.clone(),
                    )),
                }
            }
            _ => Err(ParserError::new(
                "Expected referential action (CASCADE, RESTRICT, NO ACTION, SET NULL, SET DEFAULT)",
                self.current.span.clone(),
            )),
        }
    }
}
