#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Select,
    From,
    Where,
    Join,
    On,
    As,

    Insert,
    Into,
    Values,

    Update,
    Set,

    Delete,

    Create,
    Table,
    Drop,
    Alter,

    Index,
    Unique,
    Primary,
    Key,
    Foreign,
    References,

    Not,
    Null,
    Default,
    Check,

    And,
    Or,
    In,
    Like,
    Between,
    Is,

    True,
    False,

    Group,
    Order,
    By,
    Having,
    Limit,
    Offset,

    Distinct,
    All,
    Union,
    Intersect,
    Except,

    Begin,
    Commit,
    Rollback,
    Transaction,

    With,
    Recursive,
    Case,
    When,
    Then,
    Else,
    End,

    Asc,
    Desc,
    Exists,
    Returning,

    // Literals
    IntLit(i64),
    FloatLit(f64),
    StringLit(String),

    // Identifiers
    Ident(String),
    QuotedIdent(String),

    // Operators
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    Plus,
    Minus,
    Star,
    Slash,
    Percent,

    Concat,
    Arrow,
    DoubleArrow,
    DoubleColon,

    // Punctuation
    LParen,
    RParen,
    Comma,
    Semicolon,
    Dot,
    LBracket,
    RBracket,
    LBrace,
    RBrace,

    At,
    AtGt,
    LtAt,
    AtAt,

    Hash,
    HashArrow,
    HashDoubleArrow,

    RegexMatch,
    RegexIMatch,
    RegexNotMatch,
    RegexNotIMatch,

    Parameter(u32),

    BitStringLit(String),
    HexStringLit(String),

    UnterminatedString(usize, usize),
    UnterminatedComment(usize, usize),
    UnexpectedChar(char, usize, usize),

    // Control
    Eof,

    Illegal(char, usize, usize),
}
