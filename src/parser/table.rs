use crate::ast::*;
use crate::lexer::*;
use crate::parser::parser::Parser;
use crate::parser::parser_error::ParserError;

impl<'a> Parser<'a> {
    pub fn parse_create_table(
        &mut self,
        temporary: bool,
        unlogged: bool,
    ) -> Result<CreateStmt, ParserError> {
        self.expect(TokenKind::Table)?;

        let if_not_exist = self.parse_if_not_exist()?;

        let name = self.parse_qualified_name()?;

        self.expect(TokenKind::LParen)?;

        let mut columns = vec![];
        let mut constraints = vec![];

        loop {
            if *self.current_token() == TokenKind::RParen {
                break;
            }

            if self.is_table_constraint() {
                constraints.push(self.parse_table_constraint()?);
            } else {
                columns.push(self.parse_column_def()?);
            }

            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;

        let mut inherits = vec![];
        let mut partitions = vec![];
        let mut with_options = vec![];
        let mut table_space = None;
        let mut on_commit = None;
        let mut as_query = None;

        loop {
            match self.current_token().clone() {
                TokenKind::Inherits => {
                    self.advance();
                    self.expect(TokenKind::LParen)?;
                    loop {
                        inherits.push(self.parse_qualified_name()?);
                        if !self.consume(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                }
                TokenKind::Partition => {
                    self.advance();
                    self.expect(TokenKind::By)?;

                    let kind = match self.current_token() {
                        TokenKind::Range => PartitionKind::Range,
                        TokenKind::List => PartitionKind::List,
                        TokenKind::Hash => PartitionKind::Hash,
                        _ => {
                            return Err(ParserError::new(
                                "Expected RANGE, LIST, or HASH after PARTITION BY",
                                self.current.span.clone(),
                            ));
                        }
                    };
                    self.advance();

                    self.expect(TokenKind::LParen)?;
                    let mut exprs = vec![];
                    loop {
                        exprs.push(self.parse_expr()?);
                        if !self.consume(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(TokenKind::RParen)?;

                    partitions.push(PartitionClause { kind, exprs });
                }
                TokenKind::With => {
                    self.advance();
                    self.expect(TokenKind::LParen)?;
                    loop {
                        let name = self.expect_identifier()?;
                        self.expect(TokenKind::Eq)?;
                        let value = self.parse_expr()?;
                        with_options.push(SqlOption { name, value });
                        if !self.consume(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.expect(TokenKind::RParen)?;
                }
                TokenKind::Tablespace => {
                    self.advance();
                    table_space = Some(self.expect_identifier()?);
                }
                TokenKind::On => {
                    self.advance();
                    if *self.current_token() == TokenKind::Commit {
                        self.advance();
                        match self.current_token() {
                            TokenKind::Preserve => {
                                self.advance();
                                self.consume(&TokenKind::Rows);
                                on_commit = Some(OnCommit::PreserveRows);
                            }
                            TokenKind::Delete => {
                                self.advance();
                                self.consume(&TokenKind::Rows);
                                on_commit = Some(OnCommit::DeleteRows);
                            }
                            TokenKind::Drop => {
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
                TokenKind::As => {
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

    fn is_table_constraint(&self) -> bool {
        matches!(
            self.current_token(),
            TokenKind::Primary
                | TokenKind::Foreign
                | TokenKind::Unique
                | TokenKind::Check
                | TokenKind::Constraint
        )
    }

    fn parse_column_def(&mut self) -> Result<ColumnDef, ParserError> {
        let name = self.expect_identifier()?;
        let data_type = self.parse_data_type()?;

        let collation = if *self.current_token() == TokenKind::Collate {
            self.advance();
            Some(self.expect_identifier()?)
        } else {
            None
        };

        let mut constraints = vec![];
        let mut generated = None;

        loop {
            match self.current_token().clone() {
                TokenKind::Constraint => {
                    self.advance();
                    let _constraint_name = self.expect_identifier()?;
                    constraints.push(self.parse_column_constraint()?);
                }
                TokenKind::Generated => {
                    generated = Some(self.parse_generated_column()?);
                }
                TokenKind::AutoIncrement => {
                    self.advance();
                    constraints.push(ColumnConstraint::AutoIncrement);
                }

                TokenKind::Not
                | TokenKind::Null
                | TokenKind::Default
                | TokenKind::Unique
                | TokenKind::Primary
                | TokenKind::References
                | TokenKind::Check => {
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
            TokenKind::Not => {
                self.advance();
                self.expect(TokenKind::Null)?;
                Ok(ColumnConstraint::NotNull)
            }
            TokenKind::Null => {
                self.advance();
                Ok(ColumnConstraint::Null)
            }
            TokenKind::Default => {
                self.advance();
                Ok(ColumnConstraint::Default(self.parse_expr()?))
            }
            TokenKind::Unique => {
                self.advance();
                Ok(ColumnConstraint::Unique)
            }
            TokenKind::Primary => {
                self.advance();
                self.expect(TokenKind::Key)?;
                Ok(ColumnConstraint::PrimaryKey)
            }
            TokenKind::Check => {
                self.advance();
                self.expect(TokenKind::LParen)?;
                let expr = self.parse_expr()?;
                self.expect(TokenKind::RParen)?;

                Ok(ColumnConstraint::Check(expr))
            }
            TokenKind::References => {
                self.advance();
                let table = self.parse_qualified_name()?;
                let columns = if *self.current_token() == TokenKind::LParen {
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
        self.expect(TokenKind::Generated)?;
        self.expect(TokenKind::Always)?;
        self.expect(TokenKind::As)?;
        self.expect(TokenKind::LParen)?;
        let expr = self.parse_expr()?;
        self.expect(TokenKind::RParen)?;

        self.consume(&TokenKind::Stored);

        Ok(GeneratedColumn { expr, stored: true })
    }

    fn parse_table_constraint(&mut self) -> Result<TableConstraint, ParserError> {
        let name = if *self.current_token() == TokenKind::Constraint {
            self.advance();
            Some(self.expect_identifier()?)
        } else {
            None
        };

        match self.current_token().clone() {
            TokenKind::Primary => {
                self.advance();
                self.expect(TokenKind::Key)?;
                let columns = self.parse_column_list()?;
                Ok(TableConstraint::PrimaryKey { name, columns })
            }
            TokenKind::Unique => {
                self.advance();
                let columns = self.parse_column_list()?;
                Ok(TableConstraint::Unique { name, columns })
            }
            TokenKind::Check => {
                self.advance();
                self.expect(TokenKind::LParen)?;
                let expr = self.parse_expr()?;
                self.expect(TokenKind::RParen)?;
                Ok(TableConstraint::Check { name, expr })
            }
            TokenKind::Foreign => {
                self.advance();
                self.expect(TokenKind::Key)?;
                let columns = self.parse_column_list()?;
                self.expect(TokenKind::References)?;
                let foreign_table = self.parse_qualified_name()?;
                let referred_columns = if *self.current_token() == TokenKind::LParen {
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
        if !self.consume(&TokenKind::LParen) {
            return Ok((None, None));
        }

        let prec = self.expect_int_literal()? as u8;
        let scale = if self.consume(&TokenKind::Comma) {
            Some(self.expect_int_literal()? as u8)
        } else {
            None
        };

        self.expect(TokenKind::RParen)?;
        Ok((Some(prec), scale))
    }

    fn skip_time_zone(&mut self) {
        if matches!(self.current_token(), TokenKind::With)
            || matches!(self.current_token(), TokenKind::Ident if self.source[self.current_span().start..self.current_span().end].eq_ignore_ascii_case("without"))
        {
            self.advance();
            if *self.current_token() == TokenKind::Time {
                self.advance();
                if *self.current_token() == TokenKind::Zone {
                    self.advance();
                }
            }
        }
    }

    fn parse_column_list(&mut self) -> Result<Vec<String>, ParserError> {
        self.expect(TokenKind::LParen)?;
        let mut cols = vec![self.expect_identifier()?];

        while self.consume(&TokenKind::Comma) {
            cols.push(self.expect_identifier()?);
        }

        self.expect(TokenKind::RParen)?;
        Ok(cols)
    }

    fn parse_referential_actions(
        &mut self,
    ) -> Result<(Option<ReferentialAction>, Option<ReferentialAction>), ParserError> {
        let mut on_delete = None;
        let mut on_update = None;

        for _ in 0..2 {
            if !matches!(self.current_token(), TokenKind::On) {
                break;
            }

            self.advance();

            match self.current_token().clone() {
                TokenKind::Delete => {
                    self.advance();
                    on_delete = Some(self.parse_referential_action()?);
                }
                TokenKind::Update => {
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
            TokenKind::Set => {
                self.advance(); // SET
                match self.current_token() {
                    TokenKind::Null => {
                        self.advance();
                        Ok(ReferentialAction::SetNull)
                    }
                    TokenKind::Default => {
                        self.advance();
                        Ok(ReferentialAction::SetDefault)
                    }
                    _ => Err(ParserError::new(
                        "Expected NULL or DEFAULT after SET",
                        self.current.span.clone(),
                    )),
                }
            }
            TokenKind::Cascade => {
                self.advance();
                Ok(ReferentialAction::Cascade)
            }
            TokenKind::Restrict => {
                self.advance();
                Ok(ReferentialAction::Restrict)
            }
            TokenKind::No => {
                self.advance(); // NO
                self.consume(&TokenKind::Action);
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
        self.consume(&TokenKind::Table);

        let if_exist = self.parse_if_exist()?;

        let mut names = vec![];

        loop {
            names.push(ObjectName(self.parse_qualified_name()?));
            if !self.consume(&TokenKind::Comma) {
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
        if self.consume(&TokenKind::If) {
            self.expect(TokenKind::Exists)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    // Alter table
    pub fn parse_alter_table(&mut self) -> Result<AlterTableStmt, ParserError> {
        self.consume(&TokenKind::Table);

        let if_exist = self.parse_if_exist()?;

        let name = ObjectName(self.parse_qualified_name()?);

        let mut actions = vec![];

        loop {
            actions.push(self.parse_alter_table_actions()?);

            if !self.consume(&TokenKind::Comma) {
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
            TokenKind::Add => self.parse_alter_table_add(),
            TokenKind::Drop => self.parse_alter_table_drop(),
            TokenKind::Alter => self.parse_alter_table_alter(),
            TokenKind::Set => self.parse_alter_table_set(),
            TokenKind::Reset => {
                self.advance();
                Ok(AlterTableAction::ResetOptions(
                    self.parse_reset_options_list()?,
                ))
            }
            TokenKind::Rename => self.parse_alter_table_rename(),
            TokenKind::Inherit => {
                self.advance();
                Ok(AlterTableAction::Inherit(ObjectName(
                    self.parse_qualified_name()?,
                )))
            }
            TokenKind::No => {
                self.advance();
                self.expect(TokenKind::Inherit)?;
                Ok(AlterTableAction::NoInherit(ObjectName(
                    self.parse_qualified_name()?,
                )))
            }
            TokenKind::Attach => {
                self.advance();
                self.expect(TokenKind::Partition)?;
                let partition = ObjectName(self.parse_qualified_name()?);
                self.expect(TokenKind::For)?;
                self.expect(TokenKind::Values)?;
                let for_values = self.parse_partition_bound()?;
                Ok(AlterTableAction::AttachPartition {
                    partition,
                    for_values,
                })
            }
            TokenKind::Detach => {
                self.advance();
                self.expect(TokenKind::Partition)?;
                Ok(AlterTableAction::DetachPartition(ObjectName(
                    self.parse_qualified_name()?,
                )))
            }
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
        self.consume(&TokenKind::Add);

        if self.consume(&TokenKind::Column) {
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
        self.consume(&TokenKind::Drop);

        if self.consume(&TokenKind::Column) {
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
        self.consume(&TokenKind::Alter);

        self.expect(TokenKind::Column)?;

        let name = self.expect_identifier()?;
        let action = self.parse_alter_column_actions()?;

        Ok(AlterTableAction::AlterColumn { name, action })
    }

    fn parse_alter_column_actions(&mut self) -> Result<AlterColumnAction, ParserError> {
        match self.current_token().clone() {
            TokenKind::Set => self.parse_alter_column_set(),
            TokenKind::Drop => self.parse_alter_column_drop(),
            TokenKind::Reset => self.parse_alter_column_reset(),
            TokenKind::Type => self.parse_alter_column_type(),
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
        self.consume(&TokenKind::Set);
        match self.current_token().clone() {
            TokenKind::Default => {
                self.advance();
                Ok(AlterColumnAction::SetDefault(self.parse_expr()?))
            }
            TokenKind::Not => {
                self.advance();
                self.expect(TokenKind::Null)?;
                Ok(AlterColumnAction::SetNotNull)
            }
            TokenKind::Statistics => {
                self.advance();
                let n = self.expect_int_literal()?;
                Ok(AlterColumnAction::SetStatistics(n as i64))
            }
            TokenKind::Storage => {
                self.advance();
                Ok(AlterColumnAction::SetStorage(self.parse_column_storage()?))
            }
            TokenKind::Options => {
                self.advance();
                Ok(AlterColumnAction::SetOptions(self.parse_options_list()?))
            }
            TokenKind::Data => {
                self.advance();
                self.expect(TokenKind::Type)?;
                self.parse_alter_column_type()
            }
            TokenKind::Type => {
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
        self.consume(&TokenKind::Drop);
        match self.current_token().clone() {
            TokenKind::Default => {
                self.advance();
                Ok(AlterColumnAction::DropDefault)
            }
            TokenKind::Not => {
                self.advance();
                self.expect(TokenKind::Null)?;
                Ok(AlterColumnAction::DropNotNull)
            }
            _ => Err(ParserError::new(
                format!("Unexpected token after DROP: {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }

    fn parse_alter_column_reset(&mut self) -> Result<AlterColumnAction, ParserError> {
        self.consume(&TokenKind::Reset);
        self.expect(TokenKind::LParen)?;
        let mut names = vec![];
        loop {
            names.push(self.expect_identifier()?);
            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;
        Ok(AlterColumnAction::ResetOptions(names))
    }

    fn parse_alter_column_type(&mut self) -> Result<AlterColumnAction, ParserError> {
        let data_type = self.parse_data_type()?;

        let collation = if self.consume(&TokenKind::Collate) {
            Some(self.expect_identifier()?)
        } else {
            None
        };

        let using = if self.consume(&TokenKind::Using) {
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
            TokenKind::Ident => {
                let s = self.source[self.current_span().start..self.current_span().end].to_string();
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
        self.consume(&TokenKind::Set);

        match self.current_token().clone() {
            TokenKind::Schema => {
                self.advance();
                Ok(AlterTableAction::SetSchema(self.expect_identifier()?))
            }
            TokenKind::Tablespace => {
                self.advance();
                Ok(AlterTableAction::SetTableSpace(self.expect_identifier()?))
            }
            TokenKind::Owner => {
                self.advance();
                self.expect(TokenKind::To)?;
                Ok(AlterTableAction::SetOwner(self.expect_identifier()?))
            }
            TokenKind::Options => {
                self.advance();
                Ok(AlterTableAction::SetOptions(self.parse_options_list()?))
            }
            _ => Err(ParserError::new(
                format!("Unexpected token after SET: {:?}", self.current_token()),
                self.current.span.clone(),
            )),
        }
    }

    fn parse_alter_table_rename(&mut self) -> Result<AlterTableAction, ParserError> {
        self.consume(&TokenKind::Rename);

        match self.current_token().clone() {
            TokenKind::To => {
                self.advance();
                Ok(AlterTableAction::RenameTable(self.expect_identifier()?))
            }
            TokenKind::Column => {
                self.advance();
                let old_name = self.expect_identifier()?;
                self.expect(TokenKind::To)?;
                let new_name = self.expect_identifier()?;
                Ok(AlterTableAction::RenameColumn { old_name, new_name })
            }
            TokenKind::Constraint => {
                self.advance();
                let old_name = self.expect_identifier()?;
                self.expect(TokenKind::To)?;
                let new_name = self.expect_identifier()?;
                Ok(AlterTableAction::RenameConstraint { old_name, new_name })
            }
            _ => Err(ParserError::new(
                format!(
                    "Expected TO, COLUMN or CONSTRAINT after RENAME, got {:?}",
                    self.current_token()
                ),
                self.current.span.clone(),
            )),
        }
    }

    fn parse_reset_options_list(&mut self) -> Result<Vec<String>, ParserError> {
        self.expect(TokenKind::LParen)?;
        let mut names = vec![];
        loop {
            names.push(self.expect_identifier()?);
            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;
        Ok(names)
    }

    fn parse_partition_bound(&mut self) -> Result<PartitionBound, ParserError> {
        match self.current_token().clone() {
            TokenKind::In => {
                self.advance();
                self.expect(TokenKind::LParen)?;
                let exprs = self.parse_expr_lists()?;
                self.expect(TokenKind::RParen)?;
                Ok(PartitionBound::In(exprs))
            }
            TokenKind::From => {
                self.advance();
                self.expect(TokenKind::LParen)?;
                let from = self.parse_partition_bound_values()?;
                self.expect(TokenKind::RParen)?;
                self.expect(TokenKind::To)?;
                self.expect(TokenKind::LParen)?;
                let to = self.parse_partition_bound_values()?;
                self.expect(TokenKind::RParen)?;
                Ok(PartitionBound::FromTo { from, to })
            }
            TokenKind::With => {
                self.advance();
                self.expect(TokenKind::LParen)?;
                let exprs = self.parse_expr_lists()?;
                self.expect(TokenKind::RParen)?;
                Ok(PartitionBound::With(exprs))
            }
            TokenKind::Default => {
                self.advance();
                Ok(PartitionBound::Default)
            }
            _ => Err(ParserError::new(
                format!(
                    "Expected IN, FROM, WITH or DEFAULT for partition bound, got {:?}",
                    self.current_token()
                ),
                self.current.span.clone(),
            )),
        }
    }

    fn parse_partition_bound_values(&mut self) -> Result<Vec<PartitionBoundValue>, ParserError> {
        let mut values = vec![];
        loop {
            let val = match self.current_token().clone() {
                TokenKind::Minvalue => {
                    self.advance();
                    PartitionBoundValue::Minvalue
                }
                TokenKind::Maxvalue => {
                    self.advance();
                    PartitionBoundValue::Maxvalue
                }
                _ => PartitionBoundValue::Expr(self.parse_expr()?),
            };
            values.push(val);
            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }
        Ok(values)
    }
}
