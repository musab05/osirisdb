use crate::ast::*;

/// Represents a `CREATE PROCEDURE` statement.
///
/// Procedures differ from functions in key ways:
/// - No `RETURNS` clause — procedures never return values
/// - Can manage transactions — `COMMIT`/`ROLLBACK` inside body
/// - Called with `CALL proc()` not `SELECT proc()`
/// - Cannot be used in expressions or queries
///
/// Our extensions over PostgreSQL:
/// - `TRANSACTION CONTROL` — explicitly declares transaction management
/// - `TIMEOUT n` — maximum execution time in milliseconds
/// - `IDEMPOTENT` — marks procedure as safe to retry
/// - `RETRIES n` — auto-retry count on failure
/// - `ACCESS` — visibility control at creation (same as functions)
/// - `RAISES` — declared exceptions (same as functions)
#[derive(Debug, Clone, PartialEq)]
pub struct CreateProcedureStmt {
    pub name: ObjectName,
    pub or_replace: bool,
    pub if_not_exists: bool,

    /// Parameter list — same as functions but no VARIADIC return params
    pub params: Vec<FunctionParam>,

    /// Language the procedure body is written in
    pub language: FunctionLanguage,

    /// SECURITY DEFINER | SECURITY INVOKER
    pub security: SecurityMode,

    /// SET config_param = value — runtime config options
    pub set_options: Vec<SqlOption>,

    /// ACCESS PUBLIC | PRIVATE | RESTRICTED
    /// Our extension — visibility control at creation.
    pub access: FunctionAccess,

    /// RAISES exception1, exception2
    /// Our extension — declared exceptions this procedure can raise.
    pub raises: Vec<String>,

    /// TRANSACTION CONTROL — declares that this procedure explicitly
    /// manages transactions via COMMIT/ROLLBACK in its body.
    /// Our extension — PostgreSQL has no way to declare this upfront.
    /// Useful for static analysis and tooling.
    ///
    /// Example:
    /// ```sql
    /// CREATE PROCEDURE transfer(amount NUMERIC)
    /// LANGUAGE plpgsql
    /// TRANSACTION CONTROL
    /// BEGIN
    ///     UPDATE accounts SET balance = balance - amount;
    ///     COMMIT;
    /// END;
    /// ```
    pub transaction_control: bool,

    /// TIMEOUT n — maximum execution time in milliseconds.
    /// Our extension — procedure is killed if it exceeds this time.
    /// PostgreSQL requires statement_timeout GUC set separately.
    ///
    /// Example:
    /// ```sql
    /// CREATE PROCEDURE long_job()
    /// LANGUAGE plpgsql
    /// TIMEOUT 30000
    /// BEGIN
    ///     ...
    /// END;
    /// ```
    pub timeout: Option<u64>,

    /// IDEMPOTENT — marks the procedure as safe to call multiple times
    /// with the same arguments without different side effects.
    /// Our extension — useful for retry logic and documentation.
    ///
    /// Example:
    /// ```sql
    /// CREATE PROCEDURE sync_data()
    /// LANGUAGE plpgsql
    /// IDEMPOTENT
    /// BEGIN
    ///     INSERT INTO target SELECT * FROM source
    ///     ON CONFLICT DO NOTHING;
    /// END;
    /// ```
    pub idempotent: bool,

    /// RETRIES n — number of times to automatically retry the procedure
    /// if it fails due to a serialization failure or deadlock.
    /// Our extension — PostgreSQL requires application-level retry logic.
    ///
    /// Example:
    /// ```sql
    /// CREATE PROCEDURE transfer(amount NUMERIC)
    /// LANGUAGE plpgsql
    /// RETRIES 3
    /// BEGIN
    ///     ...
    /// END;
    /// ```
    pub retries: Option<u32>,

    /// The procedure body
    pub body: FunctionBody,
}