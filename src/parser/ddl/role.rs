use crate::{
    ast::CreateRoleStmt,
    lexer::TokenKind,
    parser::{parser::Parser, parser_error::ParserError},
};

impl<'a> Parser<'a> {
    /// Parses a `CREATE ROLE` or `CREATE USER` statement.
    ///
    /// Syntax:
    /// ```sql
    /// CREATE ROLE name [IF NOT EXISTS]
    ///     [LOGIN | NOLOGIN]
    ///     [PASSWORD 'secret']
    ///     [SUPERUSER | NOSUPERUSER]
    ///     [CREATEDB | NOCREATEDB]
    ///     [CREATEROLE | NOCREATEROLE]
    ///     [INHERIT | NOINHERIT]
    ///     [REPLICATION | NOREPLICATION]
    ///     [CONNECTION LIMIT n]
    ///     [VALID UNTIL 'timestamp']
    ///     [IN ROLE role1, role2]
    ///     [ROLE role1, role2]
    /// ```
    ///
    /// `is_user` is true when called from `CREATE USER` — functionally
    /// identical to `CREATE ROLE` but stored for round-trip accuracy.
    ///
    /// The `ROLE` or `USER` keyword is consumed by the caller (`create.rs`).
    pub fn parse_create_role(&mut self, is_user: bool) -> Result<CreateRoleStmt, ParserError> {
        // Optional IF NOT EXISTS clause
        let if_not_exists = self.parse_if_not_exist()?;

        // Role name — required
        let name = self.expect_identifier()?;

        // All options default to None — only set if explicitly specified.
        // This lets the executor distinguish "not specified" from "false".
        let mut login = None;
        let mut password = None;
        let mut superuser = None;
        let mut createdb = None;
        let mut createrole = None;
        let mut inherit = None;
        let mut replication = None;
        let mut connection_limit = None;
        let mut valid_until = None;
        let mut in_role: Vec<String> = vec![];
        let mut roles: Vec<String> = vec![];

        loop {
            match self.current_token().clone() {
                // LOGIN — role can log in
                TokenKind::Login => {
                    self.advance();
                    login = Some(true);
                }
                // NOLOGIN — role cannot log in
                TokenKind::NoLogin => {
                    self.advance();
                    login = Some(false);
                }

                // PASSWORD 'secret' — sets the login password
                // = is optional, value must be a string literal
                TokenKind::Password => {
                    self.advance();
                    self.consume(&TokenKind::Eq);
                    password = Some(self.expect_string_literal()?);
                }

                // SUPERUSER — grants superuser privileges
                TokenKind::Superuser => {
                    self.advance();
                    superuser = Some(true);
                }
                // NOSUPERUSER — explicitly denies superuser
                TokenKind::NoSuperuser => {
                    self.advance();
                    superuser = Some(false);
                }

                // CREATEDB — allows creating databases
                TokenKind::CreateDb => {
                    self.advance();
                    createdb = Some(true);
                }
                // NOCREATEDB — disallows creating databases
                TokenKind::NoCreateDb => {
                    self.advance();
                    createdb = Some(false);
                }

                // CREATEROLE — allows creating other roles
                TokenKind::CreateRole => {
                    self.advance();
                    createrole = Some(true);
                }
                // NOCREATEROLE — disallows creating other roles
                TokenKind::NoCreateRole => {
                    self.advance();
                    createrole = Some(false);
                }

                // INHERIT — role inherits privileges of roles it belongs to
                TokenKind::Inherit => {
                    self.advance();
                    inherit = Some(true);
                }
                // NOINHERIT — role does not inherit privileges
                TokenKind::NoInherit => {
                    self.advance();
                    inherit = Some(false);
                }

                // REPLICATION — role can initiate streaming replication
                TokenKind::Replication => {
                    self.advance();
                    replication = Some(true);
                }
                // NOREPLICATION — role cannot initiate replication
                TokenKind::NoReplication => {
                    self.advance();
                    replication = Some(false);
                }

                // CONNECTION LIMIT n — max concurrent connections, -1 = unlimited
                TokenKind::Connection => {
                    self.advance();
                    self.expect(TokenKind::Limit)?;
                    self.consume(&TokenKind::Eq);
                    connection_limit = Some(self.expect_int()?);
                }

                // VALID UNTIL 'timestamp' — role expires after this time
                TokenKind::Valid => {
                    self.advance();
                    self.expect(TokenKind::Until)?;
                    self.consume(&TokenKind::Eq);
                    valid_until = Some(self.expect_string_literal()?);
                }

                // IN ROLE role1, role2 — immediately adds role as member of listed roles
                TokenKind::In => {
                    self.advance();
                    self.expect(TokenKind::Role)?;
                    loop {
                        in_role.push(self.expect_identifier()?);
                        if !self.consume(&TokenKind::Comma) {
                            break;
                        }
                    }
                }

                // ROLE role1, role2 — adds listed roles as members of this role
                TokenKind::Role => {
                    self.advance();
                    loop {
                        roles.push(self.expect_identifier()?);
                        if !self.consume(&TokenKind::Comma) {
                            break;
                        }
                    }
                }

                // No more recognized options — stop parsing
                _ => break,
            }
        }

        Ok(CreateRoleStmt {
            name,
            if_not_exists,
            is_user,
            login,
            password,
            superuser,
            createdb,
            createrole,
            inherit,
            replication,
            connection_limit,
            valid_until,
            in_role,
            roles,
        })
    }
}