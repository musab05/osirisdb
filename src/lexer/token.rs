use crate::lexer::spanned_token::Span;

/// Modifiers that can precede DDL statements such as `CREATE`.
///
/// These appear as part of `TokenKind::Modifier(Modifier)` and influence
/// the semantics of the DDL command that follows (e.g. `CREATE TEMPORARY TABLE`).
#[derive(Debug, Clone, PartialEq)]
pub enum Modifier {
    /// SQL `TEMPORARY` — the object exists only for the session.
    Temporary,
    /// Short form of `TEMPORARY`.
    Temp,
    /// PostgreSQL `UNLOGGED` — the table is not written to WAL.
    Unlogged,
    /// `GLOBAL` scope modifier (e.g. `GLOBAL TEMPORARY TABLE`).
    Global,
    /// `LOCAL` scope modifier (e.g. `LOCAL TEMPORARY TABLE`).
    Local,
    /// `MATERIALIZED` — used for materialized views.
    Materialized,
    /// `OR REPLACE` — replace an existing object if it exists.
    Replace,
}

/// The kind of a lexical token produced by the SQL [`Lexer`](crate::lexer::Lexer).
///
/// Variants are organised into logical groups: DML keywords, DDL keywords,
/// constraint keywords, logical operators, literals, punctuation, operators,
/// and error sentinels. Keyword variants are produced by
/// [`lookup_keyword`](crate::lexer::lookup_keyword()) when an identifier matches
/// a reserved word; otherwise `Ident` is returned.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // ── DML (Data Manipulation Language) ──
    Select,
    From,
    Where,
    Insert,
    Into,
    Values,
    Update,
    Set,
    Delete,
    Returning,
    Merge,
    // ── DDL (Data Definition Language) ──
    Create,
    Database,
    Connection,
    Encoding,
    Locale,
    Table,
    Tables,
    Schema,
    Authorization,
    Drop,
    Truncate,
    Alter,
    Reset,
    Restart,
    Identity,
    Continue,
    Index,
    View,
    Add,
    Attach,
    Column,
    Rename,
    To,
    Owner,
    Type,
    Statistics,
    Storage,
    Options,
    Data,
    Role,
    User,
    Login,
    NoLogin,
    Password,
    Superuser,
    NoSuperuser,
    Replication,
    NoReplication,
    Valid,
    Until,
    CreateDb,
    NoCreateDb,
    CreateRole,
    NoCreateRole,
    NoInherit,
    Extension,
    Version,
    // ── Constraints & Referential Integrity ──
    Constraint,
    Primary,
    Key,
    Foreign,
    References,
    Unique,
    Check,
    Not,
    Null,
    Default,
    Cascade,
    Restrict,
    Action,
    // ── Logical & Pattern-matching Operators ──
    And,
    Or,
    In,
    Out,
    Like,
    Ilike,
    Similar,
    Between,
    Is,
    Exists,
    Any,
    Some,
    Escape,
    // ── Boolean Literals ──
    True,
    False,
    // ── Grouping, Ordering & Pagination ──
    Group,
    Order,
    By,
    Having,
    Limit,
    Offset,
    Asc,
    Desc,
    Distinct,
    All,
    Nulls,
    First,
    Last,
    Fetch,
    Next,
    PercentKw,
    Tie,
    Ties,
    // ── Set Operations ──
    Union,
    Intersect,
    Except,
    // ── Transaction Control ──
    Begin,
    Commit,
    Rollback,
    Transaction,
    Savepoint,
    Release,
    // ── CTEs & Conditional Expressions ──
    With,
    Recursive,
    Case,
    When,
    Then,
    Else,
    End,
    Cast,
    If,
    // ── JOIN Clauses ──
    Join,
    On,
    As,
    Inner,
    Left,
    Right,
    Full,
    Cross,
    Outer,
    Natural,
    Using,
    Lateral,
    // ── Table Options & Advanced Clauses ──
    Inherits,
    Inherit,
    Partition,
    Range,
    List,
    Hash,
    Tablespace,
    Location,
    Collate,
    Generated,
    Always,
    Stored,
    AutoIncrement,
    Detach,
    Minvalue,
    Maxvalue,
    // ── ON COMMIT Options ──
    Preserve,
    Rows,
    // ── Utility Statements ──
    Explain,
    Analyze,
    Describe,
    /// `SHOW`
    Show,
    /// `COPY`
    Copy,
    /// `VACUUM`
    Vacuum,
    // ── Row-level Locking ──
    /// `FOR`
    For,
    /// `SHARE`
    Share,
    /// SQL `UPDATE` keyword in locking context (`FOR UPDATE`) — avoids collision with the DML `Update` variant.
    UpdateKw,
    /// `NO`
    No,
    /// `WAIT`
    Wait,
    /// `SKIP`
    Skip,
    /// `LOCKED`
    Locked,
    /// `ONLY`
    Only,
    // ── Window Functions ──
    /// `OVER`
    Over,
    Filter,
    Window,
    /// SQL `RANGE` keyword in window-frame context — avoids collision with the `Range` table-option variant.
    RangeKw,
    Preceding,
    Following,
    Current,
    Row,
    Unbounded,
    // ── UPSERT / ON CONFLICT ──
    Conflict,
    Do,
    Nothing,
    Excluded,
    // ── Sequence Management ──
    Sequence,
    Start,
    Increment,
    Cache,
    Cycle,
    Owned,
    // ── Data-type Contextual Keywords ──
    Varying,
    Precision,
    Zone,
    Time,
    Enum,
    Domain,
    Base,
    Subtype,
    Canonical,
    Preferred,
    // ── Triggers ──
    Trigger,
    Before,
    After,
    Instead,
    Of,
    InsteadOf,   // two words — handle as Ident "INSTEAD" + consume "OF"
    Each,
    Execute,
    Procedure,
    Deferrable,
    Initially,
    Deferred,
    Immediate,
    Referencing,
    Old,
    New,
    Statement,
    Function,
    Returns,
    Language,
    Volatile,
    Stable,
    Immutable,
    Strict,
    Parallel,
    Safe,
    Unsafe,
    Restricted,
    Variadic,
    Inout,
    Setof,
    Raises,
    Access,
    Definer,
    Invoker,
    Cost,
    Called,
    Input,
    Security,
    Public,
    Private,
    Void,
    Int,
    Integer,
    Bigint,
    Smallint,
    Boolean,
    Text,
    Varchar,
    Char,
    Real,
    Double,
    Numeric,
    Decimal,
    Date,
    Timestamp,
    Interval,
    Json,
    Jsonb,
    Uuid,
    Bytea,
    // ── Trigger Extensions ──
    Priority,   // PRIORITY n
    Tags,       // TAGS ('tag1', ...)
    Enabled,    // ENABLED
    Disabled,   // DISABLED
    // ── DDL Modifiers ──
    /// A DDL modifier keyword (e.g. `TEMPORARY`, `UNLOGGED`). See [`Modifier`].
    Modifier(Modifier),

    // ── Identifiers ──
    /// An unquoted identifier — the actual text is resolved via the token's [`Span`].
    Ident,
    /// A double-quoted or backtick-quoted identifier (e.g. `"my column"`).
    QuotedIdent,

    // ── Literals ──
    /// An integer literal, e.g. `42`.
    IntLit(i64),
    /// A floating-point literal, e.g. `3.14` or `1e10`.
    FloatLit(f64),
    /// A single-quoted string literal. Content is resolved via the token's [`Span`].
    StringLit,
    /// A bit-string literal, e.g. `B'1010'`.
    BitStringLit,
    /// A hex-string literal, e.g. `X'DEADBEEF'`.
    HexStringLit,
    /// A PostgreSQL `bytea` hex literal (e.g. `\x...`).
    ByteaLit,
    /// A PostgreSQL dollar-quoted string (e.g. `$$body$$`).
    DollarStringLit,
    /// A positional parameter placeholder, e.g. `$1`.
    Parameter(u32),

    // ── Comparison Operators ──
    /// `=`
    Eq,
    /// `!=` or `<>`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    // ── Arithmetic Operators ──
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*` — multiplication or wildcard select.
    Star,
    /// `/`
    Slash,
    /// `%` — modulo operator.
    Percent,
    // ── String / JSON / Cast Operators ──
    /// `||` — string concatenation.
    Concat,
    /// `->` — JSON field access (returns JSON).
    Arrow,
    /// `->>` — JSON field access (returns text).
    DoubleArrow,
    /// `::` — PostgreSQL type-cast operator.
    DoubleColon,
    /// `->` alias used in some contexts.
    MinusGt,
    // ── Bitwise Operators ──
    /// `|`
    Pipe,
    /// `^`
    Caret,
    /// `&`
    Ampersand,
    /// `~` — bitwise NOT or regex match.
    Tilde,
    /// `<<`
    ShiftLeft,
    /// `>>`
    ShiftRight,
    // ── Regex Operators (PostgreSQL) ──
    /// `~` — case-sensitive regex match.
    RegexMatch,
    /// `~*` — case-insensitive regex match.
    RegexIMatch,
    /// `!~` — negated case-sensitive regex match.
    RegexNotMatch,
    /// `!~*` — negated case-insensitive regex match.
    RegexNotIMatch,
    // ── JSONB Operators (PostgreSQL) ──
    /// `@`
    At,
    /// `@>` — JSONB contains.
    AtGt,
    /// `<@` — JSONB contained-by.
    LtAt,
    /// `@@` — full-text-search match.
    AtAt,
    /// `#` — JSONB path/delete.
    HashToken,
    /// `#>` — JSONB path access (returns JSON).
    HashArrow,
    /// `#>>` — JSONB path access (returns text).
    HashDoubleArrow,
    /// `#-` — JSONB key deletion.
    HashMinus,
    /// `?` — JSONB key existence.
    Question,
    /// `?|` — JSONB any-key existence.
    QuestionPipe,
    /// `?&` — JSONB all-key existence.
    QuestionAmp,
    // ── Punctuation ──
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `,`
    Comma,
    /// `;` — statement terminator.
    Semicolon,
    /// `.` — schema/table/column separator.
    Dot,

    // ── End of Input ──
    /// Signals that the entire input has been consumed.
    Eof,

    // ── Error Sentinels ──
    /// An illegal byte that cannot begin any valid token. Payload is the offending byte.
    Illegal(u8),
    /// A character that is valid ASCII but not expected in the current context.
    UnexpectedChar(u8),
    /// A string literal that was opened but never closed.
    UnterminatedString,
    /// A block comment (`/* ... */`) that was opened but never closed.
    UnterminatedComment,
}

/// A single lexical token produced by the [`Lexer`](crate::lexer::Lexer).
///
/// Pairs a [`TokenKind`] discriminant with a [`Span`] so downstream
/// consumers (parser, error reporter) can map every token back to its
/// exact source location.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// The semantic category of this token.
    pub kind: TokenKind,
    /// Source location metadata (byte offsets, line, column).
    pub span: Span,
}
