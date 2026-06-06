use crate::{
    ast::{BaseTypeDef, CompositeField, CreateTypeStmt, DataType, DomainConstraint, DomainDef, ObjectName, RangeTypeDef, TypeKind},
    lexer::TokenKind,
    parser::{Parser, ParserError},
};

impl<'a> Parser<'a> {
    /// Parses a `CREATE TYPE` statement.
    ///
    /// This handles defining new data types, such as composite types,
    /// `ENUM` types, `RANGE` types, or custom base types.
    pub fn parse_create_type(&mut self) -> Result<CreateTypeStmt, ParserError> {
        let name = ObjectName(self.parse_qualified_name()?);

        self.expect(TokenKind::As)?;

        let kind = match self.current_token().clone() {
            TokenKind::Enum => {
                self.advance();
                self.parse_type_enum()?
            }
            TokenKind::Range => {
                self.advance();
                self.parse_type_range()?
            }
            TokenKind::Base => {
                self.advance();
                self.parse_type_base()?
            }
            TokenKind::LParen => self.parse_type_composite()?,
            _ => {
                return Err(ParserError::new(
                    format!(
                        "Expected ENUM, RANGE, BASE, or '(', found {:?}",
                        self.current_token()
                    ),
                    self.current.span.clone(),
                ));
            }
        };

        Ok(CreateTypeStmt { name, kind })
    }

    /// Parses a `CREATE DOMAIN` statement.
    ///
    /// Domains are user-defined data types based on another underlying
    /// base type, optionally including `CHECK` constraints and `DEFAULT` values.
    pub fn parse_create_domain(&mut self) -> Result<CreateTypeStmt, ParserError> {
        let name = ObjectName(self.parse_qualified_name()?);
        self.expect(TokenKind::As)?;
        let base_type = self.parse_data_type()?;
        let kind = self.parse_type_domain(base_type)?;

        Ok(CreateTypeStmt { name, kind })
    }

    // ── Dispatch ──

    /// Parses the definition of an `ENUM` type (e.g., `AS ENUM ('val1', 'val2')`).
    fn parse_type_enum(&mut self) -> Result<TypeKind, ParserError> {
        let mut values = vec![];

        self.expect(TokenKind::LParen)?;

        loop {
            values.push(self.expect_string_literal()?);

            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;
        Ok(TypeKind::Enum(values))
    }

    /// Parses the definition of a composite type (e.g., `AS (col_name data_type, ...)`).
    fn parse_type_composite(&mut self) -> Result<TypeKind, ParserError> {
        let mut fields = vec![];

        self.expect(TokenKind::LParen)?;

        loop {
            let name = self.expect_identifier()?;
            let data_type = self.parse_data_type()?;
            let mut collation = None;

            if self.consume(&TokenKind::Collate) {
                if self.current_token() == &TokenKind::StringLit {
                    collation = Some(self.expect_string_literal()?);
                } else {
                    collation = Some(self.expect_identifier()?);
                }
            }
            fields.push(CompositeField {
                name,
                data_type,
                collation,
            });

            if !self.consume(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(TokenKind::RParen)?;
        Ok(TypeKind::Composite(fields))
    }

    /// Parses the definition of a `RANGE` type (e.g., `AS RANGE (SUBTYPE = int4)`).
    fn parse_type_range(&mut self) -> Result<TypeKind, ParserError> {
        self.expect(TokenKind::LParen)?;

        let mut range_def = RangeTypeDef {
            subtype: DataType::Custom(vec![]), // Placeholder, required field
            subtype_opclass: None,
            collation: None,
            canonical: None,
            subtype_diff: None,
        };
        let mut has_subtype = false;

        loop {
            if self.consume(&TokenKind::RParen) {
                break;
            }

            let prop_sym = self.expect_identifier()?;
            let prop = self.interner.resolve(prop_sym).to_uppercase();
            self.expect(TokenKind::Eq)?;

            match prop.as_str() {
                "SUBTYPE" => {
                    range_def.subtype = self.parse_data_type()?;
                    has_subtype = true;
                }
                "SUBTYPE_OPCLASS" => {
                    range_def.subtype_opclass = Some(self.expect_identifier()?);
                }
                "COLLATION" => {
                    if self.current_token() == &TokenKind::StringLit {
                        range_def.collation = Some(self.expect_string_literal()?);
                    } else {
                        range_def.collation = Some(self.expect_identifier()?);
                    }
                }
                "CANONICAL" => {
                    range_def.canonical = Some(self.expect_identifier()?);
                }
                "SUBTYPE_DIFF" => {
                    range_def.subtype_diff = Some(self.expect_identifier()?);
                }
                _ => {
                    return Err(ParserError::new(
                        format!("Unknown RANGE property: {}", prop),
                        self.current.span.clone(),
                    ));
                }
            }

            if !self.consume(&TokenKind::Comma) {
                self.expect(TokenKind::RParen)?;
                break;
            }
        }

        if !has_subtype {
            return Err(ParserError::new(
                "RANGE must have a SUBTYPE".to_string(),
                self.current.span.clone(),
            ));
        }

        Ok(TypeKind::Range(range_def))
    }

    /// Parses the definition of a custom base type.
    fn parse_type_base(&mut self) -> Result<TypeKind, ParserError> {
        self.expect(TokenKind::LParen)?;

        let mut base_def = BaseTypeDef {
            internal_length: None,
            alignment: None,
            storage: None,
            passed_by_value: false,
            category: None,
            preferred: false,
            default: None,
            like_type: None,
            input_func: None,
            output_func: None,
        };

        loop {
            if self.consume(&TokenKind::RParen) {
                break;
            }

            let prop_sym = self.expect_identifier()?;
            let prop = self.interner.resolve(prop_sym).to_uppercase();

            // PASSEDBYVALUE is a boolean flag that may not have an '=' following it.
            if prop == "PASSEDBYVALUE" {
                base_def.passed_by_value = true;
            } else {
                self.expect(TokenKind::Eq)?;
                match prop.as_str() {
                    "INTERNALLENGTH" => {
                        base_def.internal_length = Some(self.expect_int()?);
                    }
                    "ALIGNMENT" => {
                        base_def.alignment = Some(self.expect_identifier()?);
                    }
                    "STORAGE" => {
                        base_def.storage = Some(self.expect_identifier()?);
                    }
                    "CATEGORY" => {
                        if self.current_token() == &TokenKind::StringLit {
                            let span = self.current.span.clone();
                            let s = self.source[span.start + 1..span.end - 1].to_string();
                            base_def.category = s.chars().next();
                            self.advance();
                        } else {
                            return Err(ParserError::new(
                                format!(
                                    "CATEGORY must be a string literal, found {:?}",
                                    self.current_token()
                                ),
                                self.current.span.clone(),
                            ));
                        }
                    }
                    "PREFERRED" => {
                        if self.consume(&TokenKind::True) {
                            base_def.preferred = true;
                        } else if self.consume(&TokenKind::False) {
                            base_def.preferred = false;
                        } else {
                            let pref_sym = self.expect_identifier()?;
                            base_def.preferred = self.interner.resolve(pref_sym).to_uppercase() == "TRUE";
                        }
                    }
                    "DEFAULT" => {
                        base_def.default = Some(self.parse_expr()?);
                    }
                    "LIKE" => {
                        base_def.like_type = Some(self.parse_data_type()?);
                    }
                    "INPUT" => {
                        base_def.input_func = Some(self.expect_identifier()?);
                    }
                    "OUTPUT" => {
                        base_def.output_func = Some(self.expect_identifier()?);
                    }
                    _ => {
                        return Err(ParserError::new(
                            format!("Unknown BASE type property: {}", prop),
                            self.current.span.clone(),
                        ));
                    }
                }
            }

            if !self.consume(&TokenKind::Comma) {
                self.expect(TokenKind::RParen)?;
                break;
            }
        }

        Ok(TypeKind::Base(base_def))
    }

    /// Parses the domain-specific constraints and default values, given its base `DataType`.
    fn parse_type_domain(&mut self, base_type: DataType) -> Result<TypeKind, ParserError> {
        let mut domain_def = DomainDef {
            base_type,
            default: None,
            constraints: vec![],
            not_null: false,
        };

        loop {
            if self.consume(&TokenKind::Default) {
                domain_def.default = Some(self.parse_expr()?);
            } else if self.consume(&TokenKind::Not) {
                self.expect(TokenKind::Null)?;
                domain_def.not_null = true;
            } else if self.consume(&TokenKind::Constraint) {
                let name = self.expect_identifier()?;
                self.expect(TokenKind::Check)?;
                let check = self.parse_expr()?;
                domain_def.constraints.push(DomainConstraint {
                    name: Some(name),
                    check,
                });
            } else if self.consume(&TokenKind::Check) {
                let check = self.parse_expr()?;
                domain_def
                    .constraints
                    .push(DomainConstraint { name: None, check });
            } else {
                // Reached the end of domain modifiers
                break;
            }
        }

        Ok(TypeKind::Domain(domain_def))
    }
}
