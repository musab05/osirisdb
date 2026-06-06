use crate::ast::*;
use crate::common::symbol::Symbol;

/// Represents a full `CREATE FUNCTION` statement.
///
/// Compatible with PostgreSQL while adding:
/// - `BEGIN...END` body syntax instead of `AS $$ ... $$`
/// - `ACCESS` clause for visibility control at creation time
/// - `RAISES` clause for declaring exceptions
/// - Multi-language support
///
/// Example (PostgreSQL compatible):
/// ```sql
/// CREATE OR REPLACE FUNCTION add(a INTEGER, b INTEGER)
/// RETURNS INTEGER
/// LANGUAGE plpgsql
/// AS $$
/// BEGIN
///     RETURN a + b;
/// END;
/// $$;
/// ```
///
/// Example (our improved syntax):
/// ```sql
/// CREATE OR REPLACE FUNCTION add(a INTEGER, b INTEGER)
/// RETURNS INTEGER
/// LANGUAGE plpgsql
/// ACCESS PUBLIC
/// RAISES arithmetic_error
/// BEGIN
///     RETURN a + b;
/// END;
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct CreateFunctionStmt {
    pub name: ObjectName,
    pub or_replace: bool,
    pub if_not_exists: bool,

    /// Input, output, and variadic parameters
    pub params: Vec<FunctionParam>,

    /// Return type — required for functions
    pub returns: FunctionReturn,

    /// Language the function body is written in
    pub language: FunctionLanguage,

    /// The actual function body
    pub body: FunctionBody,

    /// Volatility — how the function behaves with respect to caching
    pub volatility: FunctionVolatility,

    /// CALLED ON NULL INPUT | RETURNS NULL ON NULL INPUT | STRICT
    pub null_behavior: NullBehavior,

    /// SECURITY DEFINER | SECURITY INVOKER
    pub security: SecurityMode,

    /// COST n — planner cost estimate, default 100
    pub cost: Option<f64>,

    /// ROWS n — estimated rows returned for set-returning functions
    pub rows: Option<f64>,

    /// SET config_param = value — runtime config for this function
    pub set_options: Vec<SqlOption>,

    /// ACCESS PUBLIC | PRIVATE | RESTRICTED
    /// Our extension — explicit visibility at creation.
    /// PostgreSQL handles this via GRANT after creation.
    ///
    /// - PUBLIC — callable by anyone (default)
    /// - PRIVATE — callable only by owner
    /// - RESTRICTED — callable only by roles explicitly granted
    pub access: FunctionAccess,

    /// RAISES exception1, exception2
    /// Our extension — declares which exceptions this function can raise.
    /// Useful for static analysis and documentation.
    ///
    /// Example:
    /// ```sql
    /// RAISES division_by_zero, null_value_not_allowed
    /// ```
    pub raises: Vec<Symbol>,

    /// PARALLEL UNSAFE | RESTRICTED | SAFE
    pub parallel: FunctionParallel,
}

/// A single function parameter.
///
/// Covers all PostgreSQL parameter modes:
/// ```sql
/// IN a INTEGER           -- input only (default)
/// OUT b TEXT             -- output only
/// INOUT c BOOLEAN        -- both input and output
/// VARIADIC d INTEGER     -- variable length input
/// a INTEGER DEFAULT 0    -- with default value
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionParam {
    /// Parameter mode — defaults to In if not specified
    pub mode: ParamMode,

    /// Parameter name — optional in PostgreSQL
    pub name: Option<Symbol>,

    /// Parameter data type — required
    pub data_type: DataType,

    /// Optional default value — only valid for IN and INOUT params
    pub default: Option<Expr>,
}

/// Parameter passing mode
#[derive(Debug, Clone, PartialEq)]
pub enum ParamMode {
    /// IN — input only, default mode
    In,
    /// OUT — output only, not passed by caller
    Out,
    /// INOUT — both input and output
    InOut,
    /// VARIADIC — variable number of arguments, must be last param
    Variadic,
}

/// What the function returns.
///
/// Covers all PostgreSQL return types:
/// ```sql
/// RETURNS INTEGER              -- scalar
/// RETURNS TABLE (id INT, ...)  -- table with named columns
/// RETURNS SETOF users          -- set of an existing type
/// RETURNS VOID                 -- nothing
/// RETURNS TRIGGER              -- for trigger functions
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum FunctionReturn {
    /// Returns a single scalar value
    Type(DataType),

    /// Returns nothing
    Void,

    /// Returns a set of values of the given type
    SetOf(DataType),

    /// Returns a table with named typed columns
    Table(Vec<FunctionParam>),

    /// Returns a trigger record — used for trigger functions
    Trigger,
}

/// The language the function body is written in.
///
/// PostgreSQL built-in languages plus common extensions.
/// `Custom(String)` covers any language not in this list
/// e.g. `plpython3u`, `plv8`, `plrust`.
#[derive(Debug, Clone, PartialEq)]
pub enum FunctionLanguage {
    /// Pure SQL — no procedural constructs
    Sql,
    /// PL/pgSQL — PostgreSQL's procedural language
    PlPgSql,
    /// PL/Python — Python via untrusted extension
    PlPython,
    /// PL/Perl — Perl via untrusted extension
    PlPerl,
    /// PL/Tcl — Tcl via extension
    PlTcl,
    /// Any other language e.g. plrust, plv8, plluau
    Custom(Symbol),
}

/// The function body — how the code is provided.
///
/// PostgreSQL supports dollar-quoted strings (`AS $$ ... $$`).
/// We also support clean `BEGIN ... END` syntax.
#[derive(Debug, Clone, PartialEq)]
pub enum FunctionBody {
    /// PostgreSQL dollar-quoted body: `AS $$ ... $$`
    /// Stored as raw string, interpreter handles parsing.
    DollarQuoted(Symbol),

    /// Our extension — clean BEGIN...END syntax.
    /// No dollar quoting needed, consistent with trigger syntax.
    ///
    /// ```sql
    /// BEGIN
    ///     RETURN a + b;
    /// END;
    /// ```
    BeginEnd(Symbol),

    /// Single SQL expression body — for simple SQL functions.
    /// ```sql
    /// AS $$ SELECT a + b $$
    /// ```
    SqlExpr(Symbol),
}

/// How the function behaves with respect to query optimization.
///
/// Affects whether the planner can cache or inline the function.
#[derive(Debug, Clone, PartialEq)]
pub enum FunctionVolatility {
    /// VOLATILE — may modify database, different results each call (default)
    Volatile,
    /// STABLE — same results within a single table scan, no side effects
    Stable,
    /// IMMUTABLE — always same results for same inputs, no side effects
    /// Planner can pre-evaluate and cache calls with constant args.
    Immutable,
}

/// How the function handles NULL inputs.
#[derive(Debug, Clone, PartialEq)]
pub enum NullBehavior {
    /// CALLED ON NULL INPUT — function is called even with NULL args (default)
    CalledOnNull,
    /// RETURNS NULL ON NULL INPUT / STRICT — returns NULL immediately if any arg is NULL
    /// Shorthand: STRICT
    Strict,
}

/// Security context the function runs in.
#[derive(Debug, Clone, PartialEq)]
pub enum SecurityMode {
    /// SECURITY INVOKER — runs with privileges of the calling user (default)
    Invoker,
    /// SECURITY DEFINER — runs with privileges of the function owner
    /// Equivalent to setuid in Unix.
    Definer,
}

/// Whether the function can be parallelized.
#[derive(Debug, Clone, PartialEq)]
pub enum FunctionParallel {
    /// PARALLEL UNSAFE — cannot be parallelized (default)
    Unsafe,
    /// PARALLEL RESTRICTED — can run in parallel but not in parallel worker
    Restricted,
    /// PARALLEL SAFE — can run freely in parallel
    Safe,
}

/// Our extension — visibility/access control at creation time.
///
/// PostgreSQL requires a separate `GRANT EXECUTE ON FUNCTION` statement.
/// We allow declaring it upfront.
#[derive(Debug, Clone, PartialEq)]
pub enum FunctionAccess {
    /// PUBLIC — callable by any role (default)
    Public,
    /// PRIVATE — callable only by the owner
    Private,
    /// RESTRICTED — callable only by explicitly granted roles
    Restricted,
}