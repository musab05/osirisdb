use crate::common::symbol::Symbol;

/// Represents a `CREATE EXTENSION` statement in the AST.
///
/// This AST node captures the parameters for loading a new extension into the
/// current database, corresponding to:
/// `CREATE EXTENSION [ IF NOT EXISTS ] extension_name [ WITH ] [ SCHEMA schema_name ] [ VERSION version ] [ CASCADE ]`.
#[derive(Debug, Clone, PartialEq)]
pub struct CreateExtensionStmt {
    /// The name of the extension to be installed.
    pub name: Symbol,
    /// If `true`, suppresses the error that would normally occur if the extension already exists.
    /// The extension is not modified in any way when this flag is set.
    pub if_not_exists: bool,
    /// Optional name of the schema in which to install the extension's objects.
    /// If not specified, the extension's default installation schema is used.
    pub schema: Option<Symbol>,
    /// Optional version of the extension to install.
    /// If not specified, the default version as defined in the extension's control file is used.
    pub version: Option<Symbol>,
    /// If `true`, automatically installs any required extensions that are not already installed.
    /// Dependencies are also installed with `CASCADE` semantics.
    pub cascade: bool,
}
