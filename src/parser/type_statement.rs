use crate::{
    ast::{CreateTypeStmt, DataType, ObjectName, TypeKind}, lexer::TokenKind, parser::{Parser, ParserError}
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
        TokenKind::Enum       => { self.advance(); self.parse_type_enum()? }
        TokenKind::Range      => { self.advance(); self.parse_type_range()? }
        TokenKind::Base       => { self.advance(); self.parse_type_base()? }
        TokenKind::LParen     => self.parse_type_composite()?,  // no keyword, just (
        _ => return Err(...)
    };

    Ok(CreateTypeStmt { name, kind })
    }

    /// Parses a `CREATE DOMAIN` statement.
    ///
    /// Domains are user-defined data types based on another underlying
    /// base type, optionally including `CHECK` constraints and `DEFAULT` values.
    pub fn parse_create_domain(&mut self) -> Result<CreateTypeStmt, ParserError> {
        todo!()
    }

    // ── Dispatch ──

    /// Parses the definition of an `ENUM` type (e.g., `AS ENUM ('val1', 'val2')`).
    fn parse_type_enum(&mut self) -> Result<TypeKind, ParserError> {
        todo!()
    }

    /// Parses the definition of a composite type (e.g., `AS (col_name data_type, ...)`).
    fn parse_type_composite(&mut self) -> Result<TypeKind, ParserError> {
        todo!()
    }

    /// Parses the definition of a `RANGE` type (e.g., `AS RANGE (SUBTYPE = int4)`).
    fn parse_type_range(&mut self) -> Result<TypeKind, ParserError> {
        todo!()
    }

    /// Parses the definition of a custom base type.
    fn parse_type_base(&mut self) -> Result<TypeKind, ParserError> {
        todo!()
    }

    /// Parses the domain-specific constraints and default values, given its base `DataType`.
    fn parse_type_domain(&mut self, base_type: DataType) -> Result<TypeKind, ParserError> {
        todo!()
    }
}
