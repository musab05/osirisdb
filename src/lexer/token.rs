#[derive(Debug, Clone, PartialEq)]
pub enum Modifier {
    Temporary,
    Temp,
    Unlogged,
    Global,
    Local,
    Materialized,
    Replace,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // ─────────────────────────────────────────────────
    // Keywords — DML
    // ─────────────────────────────────────────────────
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

    // ─────────────────────────────────────────────────
    // Keywords — DDL
    // ─────────────────────────────────────────────────
    Create,
    Table,
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
    Column,
    Rename,
    To,
    Owner,
    Type,
    Statistics,
    Storage,
    Options,
    Data,

    // ─────────────────────────────────────────────────
    // Keywords — Constraints & References
    // ─────────────────────────────────────────────────
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

    // ─────────────────────────────────────────────────
    // Keywords — Logical & Comparison
    // ─────────────────────────────────────────────────
    And,
    Or,
    In,
    Like,
    Ilike,
    Similar,
    Between,
    Is,
    Exists,
    Any,
    Some,
    Escape,

    // ─────────────────────────────────────────────────
    // Keywords — Boolean Literals
    // ─────────────────────────────────────────────────
    True,
    False,

    // ─────────────────────────────────────────────────
    // Keywords — Grouping, Ordering & Pagination
    // ─────────────────────────────────────────────────
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

    // ─────────────────────────────────────────────────
    // Keywords — Set Operations
    // ─────────────────────────────────────────────────
    Union,
    Intersect,
    Except,

    // ─────────────────────────────────────────────────
    // Keywords — Transaction
    // ─────────────────────────────────────────────────
    Begin,
    Commit,
    Rollback,
    Transaction,
    Savepoint,
    Release,

    // ─────────────────────────────────────────────────
    // Keywords — CTE & Conditional
    // ─────────────────────────────────────────────────
    With,
    Recursive,
    Case,
    When,
    Then,
    Else,
    End,
    Cast,
    If,

    // ─────────────────────────────────────────────────
    // Keywords — Joins
    // ─────────────────────────────────────────────────
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

    // ─────────────────────────────────────────────────
    // Keywords — Table Options & Clauses
    // ─────────────────────────────────────────────────
    Inherits,
    Partition,
    Range,
    List,
    Tablespace,
    Collate,
    Generated,
    Always,
    Stored,
    AutoIncrement,

    // ─────────────────────────────────────────────────
    // Keywords — ON COMMIT Options
    // ─────────────────────────────────────────────────
    Preserve,
    Rows,

    // ─────────────────────────────────────────────────
    // Keywords — Locking
    // ─────────────────────────────────────────────────
    For,
    Share,
    UpdateKw,
    No,
    Wait,
    Skip,
    Locked,
    Only,

    // ─────────────────────────────────────────────────
    // Keywords — Window Functions
    // ─────────────────────────────────────────────────
    Over,
    Filter,
    Window,
    RangeKw,
    Preceding,
    Following,
    Current,
    Row,
    Unbounded,

    // ─────────────────────────────────────────────────
    // Keywords — UPSERT / Conflict
    // ─────────────────────────────────────────────────
    Conflict,
    Do,
    Nothing,
    Excluded,

    // ─────────────────────────────────────────────────
    // Keywords — Data Type Names (used contextually)
    // ─────────────────────────────────────────────────
    Varying,
    Precision,
    Zone,
    Time,

    // ─────────────────────────────────────────────────
    // Modifiers
    // ─────────────────────────────────────────────────
    Modifier(Modifier),

    // ─────────────────────────────────────────────────
    // Identifiers
    // ─────────────────────────────────────────────────
    Ident(String),
    QuotedIdent(String),

    // ─────────────────────────────────────────────────
    // Literals / Constants
    // ─────────────────────────────────────────────────
    IntLit(i64),
    FloatLit(f64),
    StringLit(String),
    BitStringLit(String),
    HexStringLit(String),
    ByteaLit(Vec<u8>),
    DollarStringLit(String),
    Parameter(u32),

    // ─────────────────────────────────────────────────
    // Operators — Comparison
    // ─────────────────────────────────────────────────
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    // ─────────────────────────────────────────────────
    // Operators — Arithmetic
    // ─────────────────────────────────────────────────
    Plus,
    Minus,
    Star,
    Slash,
    Percent,

    // ─────────────────────────────────────────────────
    // Operators — String / JSON / Cast
    // ─────────────────────────────────────────────────
    Concat,
    Arrow,
    DoubleArrow,
    DoubleColon,
    MinusGt,

    // ─────────────────────────────────────────────────
    // Operators — Bitwise
    // ─────────────────────────────────────────────────
    Pipe,
    Caret,
    Ampersand,
    Tilde,
    ShiftLeft,
    ShiftRight,

    // ─────────────────────────────────────────────────
    // Operators — Regex
    // ─────────────────────────────────────────────────
    RegexMatch,
    RegexIMatch,
    RegexNotMatch,
    RegexNotIMatch,

    // ─────────────────────────────────────────────────
    // Operators — Containment / JSONB
    // ─────────────────────────────────────────────────
    At,
    AtGt,
    LtAt,
    AtAt,
    Hash,
    HashArrow,
    HashDoubleArrow,
    HashMinus,
    Question,
    QuestionPipe,
    QuestionAmp,

    // ─────────────────────────────────────────────────
    // Delimiters / Separators / Punctuation
    // ─────────────────────────────────────────────────
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Semicolon,
    Dot,

    // ─────────────────────────────────────────────────
    // End Of File
    // ─────────────────────────────────────────────────
    Eof,

    // ─────────────────────────────────────────────────
    // Invalid / Unknown / Error Tokens
    // ─────────────────────────────────────────────────
    Illegal(char, usize, usize),
    UnexpectedChar(char, usize, usize),
    UnterminatedString(usize, usize),
    UnterminatedComment(usize, usize),
}
