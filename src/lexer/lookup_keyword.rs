use crate::lexer::token::{Modifier, Token};

pub fn lookup_keyword(word: &str) -> Token {
    match word.to_uppercase().as_str() {
        // ── DML ──
        "SELECT" => Token::Select,
        "FROM" => Token::From,
        "WHERE" => Token::Where,
        "INSERT" => Token::Insert,
        "INTO" => Token::Into,
        "VALUES" => Token::Values,
        "UPDATE" => Token::Update,
        "SET" => Token::Set,
        "DELETE" => Token::Delete,
        "RETURNING" => Token::Returning,

        // ── DDL ──
        "CREATE" => Token::Create,
        "TABLE" => Token::Table,
        "DROP" => Token::Drop,
        "TRUNCATE" => Token::Truncate,
        "ALTER" => Token::Alter,
        "RESTART" => Token::Restart,
        "IDENTITY" => Token::Identity,
        "CONTINUE" => Token::Continue,
        "INDEX" => Token::Index,
        "VIEW" => Token::View,
        "ADD" => Token::Add,
        "COLUMN" => Token::Column,
        "RENAME" => Token::Rename,
        "TO" => Token::To,
        "OWNER" => Token::Owner,
        "TYPE" => Token::Type,

        // ── Constraints & References ──
        "CONSTRAINT" => Token::Constraint,
        "PRIMARY" => Token::Primary,
        "KEY" => Token::Key,
        "FOREIGN" => Token::Foreign,
        "REFERENCES" => Token::References,
        "UNIQUE" => Token::Unique,
        "CHECK" => Token::Check,
        "NOT" => Token::Not,
        "NULL" => Token::Null,
        "DEFAULT" => Token::Default,
        "CASCADE" => Token::Cascade,
        "RESTRICT" => Token::Restrict,
        "ACTION" => Token::Action,

        // ── Logical & Comparison ──
        "AND" => Token::And,
        "OR" => Token::Or,
        "IN" => Token::In,
        "LIKE" => Token::Like,
        "ILIKE" => Token::Ilike,
        "SIMILAR" => Token::Similar,
        "BETWEEN" => Token::Between,
        "IS" => Token::Is,
        "EXISTS" => Token::Exists,
        "ANY" => Token::Any,
        "SOME" => Token::Some,
        "ESCAPE" => Token::Escape,

        // ── Boolean Literals ──
        "TRUE" => Token::True,
        "FALSE" => Token::False,

        // ── Grouping, Ordering & Pagination ──
        "GROUP" => Token::Group,
        "ORDER" => Token::Order,
        "BY" => Token::By,
        "HAVING" => Token::Having,
        "LIMIT" => Token::Limit,
        "OFFSET" => Token::Offset,
        "ASC" => Token::Asc,
        "DESC" => Token::Desc,
        "DISTINCT" => Token::Distinct,
        "ALL" => Token::All,
        "NULLS" => Token::Nulls,
        "FIRST" => Token::First,
        "LAST" => Token::Last,
        "FETCH" => Token::Fetch,
        "NEXT" => Token::Next,
        "PERCENT" => Token::PercentKw,
        "TIE" => Token::Tie,
        "TIES" => Token::Ties,

        // ── Set Operations ──
        "UNION" => Token::Union,
        "INTERSECT" => Token::Intersect,
        "EXCEPT" => Token::Except,

        // ── Transaction ──
        "BEGIN" => Token::Begin,
        "COMMIT" => Token::Commit,
        "ROLLBACK" => Token::Rollback,
        "TRANSACTION" => Token::Transaction,
        "SAVEPOINT" => Token::Savepoint,
        "RELEASE" => Token::Release,

        // ── CTE & Conditional ──
        "WITH" => Token::With,
        "RECURSIVE" => Token::Recursive,
        "CASE" => Token::Case,
        "WHEN" => Token::When,
        "THEN" => Token::Then,
        "ELSE" => Token::Else,
        "END" => Token::End,
        "CAST" => Token::Cast,
        "IF" => Token::If,

        // ── Joins ──
        "JOIN" => Token::Join,
        "ON" => Token::On,
        "AS" => Token::As,
        "INNER" => Token::Inner,
        "LEFT" => Token::Left,
        "RIGHT" => Token::Right,
        "FULL" => Token::Full,
        "CROSS" => Token::Cross,
        "OUTER" => Token::Outer,
        "NATURAL" => Token::Natural,
        "USING" => Token::Using,
        "LATERAL" => Token::Lateral,

        // ── Table Options & Clauses ──
        "INHERITS" => Token::Inherits,
        "PARTITION" => Token::Partition,
        "RANGE" => Token::Range,
        "LIST" => Token::List,
        "HASH" => Token::Hash,
        "TABLESPACE" => Token::Tablespace,
        "COLLATE" => Token::Collate,
        "GENERATED" => Token::Generated,
        "ALWAYS" => Token::Always,
        "STORED" => Token::Stored,
        "AUTOINCREMENT" | "AUTO_INCREMENT" => Token::AutoIncrement,

        // ── ON COMMIT Options ──
        "PRESERVE" => Token::Preserve,
        "ROWS" => Token::Rows,

        // ── Locking ──
        "FOR" => Token::For,
        "SHARE" => Token::Share,
        "NO" => Token::No,
        "WAIT" => Token::Wait,
        "SKIP" => Token::Skip,
        "LOCKED" => Token::Locked,
        "ONLY" => Token::Only,

        // ── Window Functions ──
        "OVER" => Token::Over,
        "FILTER" => Token::Filter,
        "WINDOW" => Token::Window,
        "PRECEDING" => Token::Preceding,
        "FOLLOWING" => Token::Following,
        "CURRENT" => Token::Current,
        "ROW" => Token::Row,
        "UNBOUNDED" => Token::Unbounded,

        // ── UPSERT / Conflict ──
        "CONFLICT" => Token::Conflict,
        "DO" => Token::Do,
        "NOTHING" => Token::Nothing,
        "EXCLUDED" => Token::Excluded,

        // ── Data Type Contextual Keywords ──
        "VARYING" => Token::Varying,
        "PRECISION" => Token::Precision,
        "ZONE" => Token::Zone,
        "TIME" => Token::Time,

        // ── Modifiers ──
        "TEMPORARY" => Token::Modifier(Modifier::Temporary),
        "TEMP" => Token::Modifier(Modifier::Temp),
        "UNLOGGED" => Token::Modifier(Modifier::Unlogged),
        "GLOBAL" => Token::Modifier(Modifier::Global),
        "LOCAL" => Token::Modifier(Modifier::Local),
        "MATERIALIZED" => Token::Modifier(Modifier::Materialized),
        "REPLACE" => Token::Modifier(Modifier::Replace),

        // ── Fallback: identifier ──
        _ => Token::Ident(word.to_string()),
    }
}
