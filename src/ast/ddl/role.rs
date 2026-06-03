/// Represents a `CREATE ROLE` or `CREATE USER` statement.
///
/// In SQL, roles are entities that can own database objects and have database privileges.
/// A role can act as a "user", a "group", or both depending on its configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateRoleStmt {
    /// The name of the role to be created.
    pub name: String,

    /// If true, do not throw an error if a role with the same name already exists (`IF NOT EXISTS`).
    pub if_not_exists: bool,

    /// Distinguishes between `CREATE USER` (true) and `CREATE ROLE` (false).
    /// Note: `CREATE USER` typically implies the `LOGIN` privilege by default.
    pub is_user: bool,

    /// Whether the role is allowed to log in (`LOGIN` / `NOLOGIN`).
    pub login: Option<bool>,

    /// The password associated with the role, if any.
    pub password: Option<String>,

    /// Whether the role is a superuser bypassing all permission checks (`SUPERUSER` / `NOSUPERUSER`).
    pub superuser: Option<bool>,

    /// Whether the role is allowed to create new databases (`CREATEDB` / `NOCREATEDB`).
    pub createdb: Option<bool>,

    /// Whether the role is allowed to create, alter, and drop other roles (`CREATEROLE` / `NOCREATEROLE`).
    pub createrole: Option<bool>,

    /// Whether the role inherits the privileges of roles it is a member of (`INHERIT` / `NOINHERIT`).
    pub inherit: Option<bool>,

    /// Whether the role is allowed to initiate streaming replication (`REPLICATION` / `NOREPLICATION`).
    pub replication: Option<bool>,

    /// How many concurrent connections the role can make. Negative values usually signify no limit.
    pub connection_limit: Option<i64>,

    /// A timestamp or string indicating when the role's privileges expire (`VALID UNTIL`).
    pub valid_until: Option<String>,

    /// Lists roles to which the new role will be immediately added as a new member (`IN ROLE`).
    pub in_role: Vec<String>,

    /// Lists roles which will be immediately added as members of the new role (`ROLE`).
    pub roles: Vec<String>,
}
