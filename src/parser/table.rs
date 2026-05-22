use crate::ast::*;
use crate::lexer::*;
use crate::parser::parser::Parser;
use crate::parser::parser_error::ParserError;

impl Parser {
    pub fn parse_create_table(
        &mut self,
        temporary: bool,
        unlogged: bool,
    ) -> Result<CreateStmt, ParserError> {
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
                Token::Inherits => {
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
                Token::Partition => {
                    self.advance();
                    self.expect(Token::By)?;

                    let kind = match self.current_token() {
                        Token::Range => PartitionKind::Range,
                        Token::List => PartitionKind::List,
                        Token::Hash => PartitionKind::Hash,
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
                Token::Tablespace => {
                    self.advance();
                    table_space = Some(self.expect_identifier()?);
                }
                Token::On => {
                    self.advance();
                    if *self.current_token() == Token::Commit {
                        self.advance();
                        match self.current_token() {
                            Token::Preserve => {
                                self.advance();
                                self.consume(&Token::Rows);
                                on_commit = Some(OnCommit::PreserveRows);
                            }
                            Token::Delete => {
                                self.advance();
                                self.consume(&Token::Rows);
                                on_commit = Some(OnCommit::DeleteRows);
                            }
                            Token::Drop => {
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
        if *self.current_token() == Token::If {
            self.advance();
            self.expect(Token::Not)?;
            self.expect(Token::Exists)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn is_table_constraint(&self) -> bool {
        matches!(
            self.current_token(),
            Token::Primary | Token::Foreign | Token::Unique | Token::Check | Token::Constraint
        )
    }

    fn parse_column_def(&mut self) -> Result<ColumnDef, ParserError> {
        let name = self.expect_identifier()?;
        let data_type = self.parse_data_type()?;

        let collation = if *self.current_token() == Token::Collate {
            self.advance();
            Some(self.expect_identifier()?)
        } else {
            None
        };

        let mut constraints = vec![];
        let mut generated = None;

        loop {
            match self.current_token().clone() {
                Token::Constraint => {
                    self.advance();
                    let _constraint_name = self.expect_identifier()?;
                    constraints.push(self.parse_column_constraint()?);
                }
                Token::Generated => {
                    generated = Some(self.parse_generated_column()?);
                }
                Token::AutoIncrement => {
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
        self.expect(Token::Generated)?;
        self.expect(Token::Always)?;
        self.expect(Token::As)?;
        self.expect(Token::LParen)?;
        let expr = self.parse_expr()?;
        self.expect(Token::RParen)?;

        self.consume(&Token::Stored);

        Ok(GeneratedColumn { expr, stored: true })
    }

    fn parse_table_constraint(&mut self) -> Result<TableConstraint, ParserError> {
        let name = if *self.current_token() == Token::Constraint {
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
        if matches!(self.current_token(), Token::With)
            || matches!(self.current_token(), Token::Ident(k) if k.eq_ignore_ascii_case("without"))
        {
            self.advance();
            if *self.current_token() == Token::Time {
                self.advance();
                if *self.current_token() == Token::Zone {
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
                Token::Update => {
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
            Token::Cascade => {
                self.advance();
                Ok(ReferentialAction::Cascade)
            }
            Token::Restrict => {
                self.advance();
                Ok(ReferentialAction::Restrict)
            }
            Token::No => {
                self.advance(); // NO
                self.consume(&Token::Action);
                Ok(ReferentialAction::NoAction)
            }
            _ => Err(ParserError::new(
                "Expected referential action (CASCADE, RESTRICT, NO ACTION, SET NULL, SET DEFAULT)",
                self.current.span.clone(),
            )),
        }
    }

    // Drop table
    pub fn parse_drop_table(&mut self, temporary: bool) -> Result<DropTableStmt, ParserError> {
        self.consume(&Token::Table);

        let if_exist = self.parse_if_exist()?;

        let mut names = vec![];

        loop {
            names.push(ObjectName(self.parse_qualified_name()?));
            if !self.consume(&Token::Comma) {
                break;
            }
        }

        let behaviour = self.parse_drop_behaviour();

        Ok(DropTableStmt {
            if_exist,
            temporary,
            names,
            behaviour,
        })
    }

    fn parse_if_exist(&mut self) -> Result<bool, ParserError> {
        if self.consume(&Token::If) {
            self.expect(Token::Exists)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // Alter table
    pub fn parse_alter_table(&mut self) -> Result<AlterTableStmt, ParserError> {
        self.consume(&Token::Table);

        let if_exist = self.parse_if_exist()?;

        let name = ObjectName(self.parse_qualified_name()?);

        let mut actions = vec![];

        loop {
            actions.push(self.parse_alter_table_actions()?);

            if !self.consume(&Token::Comma) {
                break;
            }
        }

        Ok(AlterTableStmt {
            if_exist,
            name,
            actions,
        })
    }

    fn parse_alter_table_actions(&mut self) -> Result<AlterTableAction, ParserError> {
        match self.current_token().clone() {
            Token::Add => self.parse_alter_table_add(),
            Token::Drop => self.parse_alter_table_drop(),
            Token::Alter => self.parse_alter_table_alter(),
            Token::Set => self.parse_alter_table_set(),
            _ => Err(ParserError::new(
                format!(
                    "Unexpected token in ALTER TABLE: {:?}",
                    self.current_token()
                ),
                self.current.span.clone(),
            )),
        }
    }

    fn parse_alter_table_add(&mut self) -> Result<AlterTableAction, ParserError> {
        self.consume(&Token::Add);

        if self.consume(&Token::Column) {
            let if_not_exist = self.parse_if_not_exist()?;
            let column = self.parse_column_def()?;
            Ok(AlterTableAction::AddColumn {
                if_not_exist,
                column,
            })
        } else if self.is_table_constraint() {
            let constraint = self.parse_table_constraint()?;
            Ok(AlterTableAction::AddConstraint(constraint))
        } else {
            Err(ParserError::new(
                format!(
                    "Expected COLUMN or constraint after ADD, got {:?}",
                    self.current_token()
                ),
                self.current.span.clone(),
            ))
        }
    }

    fn parse_alter_table_drop(&mut self) -> Result<AlterTableAction, ParserError> {
        self.consume(&Token::Drop);

        if self.consume(&Token::Column) {
            let if_exist = self.parse_if_exist()?;
            let name = self.expect_identifier()?;
            let behaviour = self.parse_drop_behaviour();
            Ok(AlterTableAction::DropColumn {
                if_exist,
                name,
                behaviour,
            })
        } else if self.is_table_constraint() {
            let if_exist = self.parse_if_exist()?;
            let name = self.expect_identifier()?;
            let behaviour = self.parse_drop_behaviour();
            Ok(AlterTableAction::DropConstraint {
                if_exist,
                name,
                behaviour,
            })
        } else {
            Err(ParserError::new(
                format!(
                    "Expected COLUMN or constraint after DROP, got {:?}",
                    self.current_token()
                ),
                self.current.span.clone(),
            ))
        }
    }

    fn parse_alter_table_alter(&mut self) -> Result<AlterTableAction, ParserError> {
        self.consume(&Token::Alter);

        self.expect(Token::Column)?;

        let name = self.expect_identifier()?;
        let action = self.parse_alter_column_actions()?;

        Ok(AlterTableAction::AlterColumn { name, action })
    }

    fn parse_alter_column_actions(&mut self) -> Result<AlterColumnAction, ParserError> {
        match self.current_token().clone() {
            Token::Set => self.parse_alter_column_set(),
            Token::Drop => self.parse_alter_column_drop(),
            Token::Reset => self.parse_alter_column_reset(),
            Token::Type => self.parse_alter_column_type(),
            _ => Err(ParserError::new(
                format!(
                    "Unexpected token in ALTER COLUMN: {:?}",
                    self.current_token()
                ),
                self.current.span.clone(),
            )),
        }
    }

    fn parse_alter_column_set(&mut self) -> Result<AlterColumnAction, ParserError> {
        self.consume(&Token::Set);
        match self.current_token().clone() {
            Token::Default => {
                self.advance();
                Ok(AlterColumnAction::SetDefault(self.parse_expr()?))
            }
            Token::Not => {
                self.advance();
                self.expect(Token::Null)?;
                Ok(AlterColumnAction::SetNotNull)
            }
            Token::Statistics => {
                self.advance();
                let n = self.expect_int_literal()?;
                Ok(AlterColumnAction::SetStatistics(n as i64))
            }
            Token::Storage => {
                self.advance();
                Ok(AlterColumnAction::SetStorage(self.parse_column_storage()?))
            }
            Token::Options => {
                self.advance();
                Ok(AlterColumnAction::SetOptions(self.parse_options_list()?))
            }
            Token::Data => {
                self.advance();
                self.expect(Token::Type)?;
                self.parse_alter_column_type()
            }
            Token::Type => {
                self.advance();
                self.parse_alter_column_type()
            }
            _ => Err(ParserError::new(
                format!("Unexpected token after SET: {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }

    fn parse_alter_column_drop(&mut self) -> Result<AlterColumnAction, ParserError> {
        self.consume(&Token::Drop);
        match self.current_token().clone() {
            Token::Default => {
                self.advance();
                Ok(AlterColumnAction::DropDefault)
            }
            Token::Not => {
                self.advance();
                self.expect(Token::Null)?;
                Ok(AlterColumnAction::DropNotNull)
            }
            _ => Err(ParserError::new(
                format!("Unexpected token after DROP: {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }

    fn parse_alter_column_reset(&mut self) -> Result<AlterColumnAction, ParserError> {
        self.consume(&Token::Reset);
        self.expect(Token::LParen)?;
        let mut names = vec![];
        loop {
            names.push(self.expect_identifier()?);
            if !self.consume(&Token::Comma) {
                break;
            }
        }
        self.expect(Token::RParen)?;
        Ok(AlterColumnAction::ResetOptions(names))
    }

    fn parse_alter_column_type(&mut self) -> Result<AlterColumnAction, ParserError> {
        let data_type = self.parse_data_type()?;

        let collation = if self.consume(&Token::Collate) {
            Some(self.expect_identifier()?)
        } else {
            None
        };

        let using = if self.consume(&Token::Using) {
            Some(self.parse_expr()?)
        } else {
            None
        };

        Ok(AlterColumnAction::SetType {
            data_type,
            collation,
            using,
        })
    }

    fn parse_column_storage(&mut self) -> Result<ColumnStorage, ParserError> {
        match self.current_token().clone() {
            Token::Ident(s) => {
                let storage = match s.to_uppercase().as_str() {
                    "PLAIN" => ColumnStorage::Plain,
                    "EXTERNAL" => ColumnStorage::External,
                    "EXTENDED" => ColumnStorage::Extended,
                    "MAIN" => ColumnStorage::Main,
                    _ => {
                        return Err(ParserError::new(
                            format!("Unknown storage type: {}", s),
                            self.current.span.clone(),
                        ));
                    }
                };
                self.advance();
                Ok(storage)
            }
            _ => Err(ParserError::new(
                format!("Expected storage type, got {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }

    fn parse_alter_table_set(&mut self) -> Result<AlterTableAction, ParserError> {
        todo!()
    }
}
