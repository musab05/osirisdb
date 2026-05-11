use crate::lexer::token::Token;

pub fn lookup_keyword(word: &str) -> Token {
    match word.to_uppercase().as_str() {
        "SELECT" => Token::Select,
        "FROM" => Token::From,
        "WHERE" => Token::Where,
        "JOIN" => Token::Join,
        "ON" => Token::On,
        "AS" => Token::As,

        "INSERT" => Token::Insert,
        "INTO" => Token::Into,
        "VALUES" => Token::Values,

        "UPDATE" => Token::Update,
        "SET" => Token::Set,

        "DELETE" => Token::Delete,

        "CREATE" => Token::Create,
        "TABLE" => Token::Table,
        "DROP" => Token::Drop,
        "ALTER" => Token::Alter,

        "INDEX" => Token::Index,
        "UNIQUE" => Token::Unique,
        "PRIMARY" => Token::Primary,
        "KEY" => Token::Key,
        "FOREIGN" => Token::Foreign,
        "REFERENCES" => Token::References,

        "NOT" => Token::Not,
        "NULL" => Token::Null,
        "DEFAULT" => Token::Default,
        "CHECK" => Token::Check,

        "AND" => Token::And,
        "OR" => Token::Or,
        "IN" => Token::In,
        "LIKE" => Token::Like,
        "BETWEEN" => Token::Between,
        "IS" => Token::Is,

        "TRUE" => Token::True,
        "FALSE" => Token::False,

        "GROUP" => Token::Group,
        "ORDER" => Token::Order,
        "BY" => Token::By,
        "HAVING" => Token::Having,
        "LIMIT" => Token::Limit,
        "OFFSET" => Token::Offset,

        "DISTINCT" => Token::Distinct,
        "ALL" => Token::All,
        "UNION" => Token::Union,
        "INTERSECT" => Token::Intersect,
        "EXCEPT" => Token::Except,

        "BEGIN" => Token::Begin,
        "COMMIT" => Token::Commit,
        "ROLLBACK" => Token::Rollback,
        "TRANSACTION" => Token::Transaction,

        "WITH" => Token::With,
        "RECURSIVE" => Token::Recursive,
        "CASE" => Token::Case,
        "WHEN" => Token::When,
        "THEN" => Token::Then,
        "ELSE" => Token::Else,
        "END" => Token::End,

        "ASC" => Token::Asc,
        "DESC" => Token::Desc,
        "EXISTS" => Token::Exists,
        "RETURNING" => Token::Returning,

        _ => Token::Ident(word.to_string()),
    }
}