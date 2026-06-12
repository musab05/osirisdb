use osirisdb::lexer::{Lexer, TokenKind};

fn lex(input: &str) -> Vec<TokenKind> {
    let mut lexer = Lexer::new(input);
    let mut tokens = Vec::new();
    loop {
        let token = lexer.next_token();
        let kind = token.kind.clone();
        tokens.push(kind);
        if token.kind == TokenKind::Eof {
            break;
        }
    }
    tokens
}

// ── Keywords ──────────────────────────────────────────────────────────────────

#[test]
fn test_create_keyword() {
    assert_eq!(lex("CREATE"), vec![TokenKind::Create, TokenKind::Eof]);
}

#[test]
fn test_database_keyword() {
    assert_eq!(lex("DATABASE"), vec![TokenKind::Database, TokenKind::Eof]);
}

#[test]
fn test_create_database_keywords() {
    assert_eq!(
        lex("CREATE DATABASE"),
        vec![TokenKind::Create, TokenKind::Database, TokenKind::Eof,]
    );
}

#[test]
fn test_keywords_case_insensitive() {
    assert_eq!(lex("create database"), lex("CREATE DATABASE"));
    assert_eq!(lex("Create Database"), lex("CREATE DATABASE"));
}

// ── Clauses ───────────────────────────────────────────────────────────────────

#[test]
fn test_if_not_exists_tokens() {
    assert_eq!(
        lex("IF NOT EXISTS"),
        vec![
            TokenKind::If,
            TokenKind::Not,
            TokenKind::Exists,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn test_owner_keyword() {
    assert_eq!(lex("OWNER"), vec![TokenKind::Owner, TokenKind::Eof]);
}

#[test]
fn test_encoding_keyword() {
    assert_eq!(lex("ENCODING"), vec![TokenKind::Encoding, TokenKind::Eof]);
}

#[test]
fn test_locale_keyword() {
    assert_eq!(lex("LOCALE"), vec![TokenKind::Locale, TokenKind::Eof]);
}

#[test]
fn test_tablespace_keyword() {
    assert_eq!(
        lex("TABLESPACE"),
        vec![TokenKind::Tablespace, TokenKind::Eof]
    );
}

#[test]
fn test_connection_limit_keywords() {
    assert_eq!(
        lex("CONNECTION LIMIT"),
        vec![TokenKind::Connection, TokenKind::Limit, TokenKind::Eof,]
    );
}

// ── Full statements ───────────────────────────────────────────────────────────

#[test]
fn test_full_create_database_minimal() {
    assert_eq!(
        lex("CREATE DATABASE mydb;"),
        vec![
            TokenKind::Create,
            TokenKind::Database,
            TokenKind::Ident,
            TokenKind::Semicolon,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn test_full_create_database_if_not_exists() {
    assert_eq!(
        lex("CREATE DATABASE IF NOT EXISTS mydb;"),
        vec![
            TokenKind::Create,
            TokenKind::Database,
            TokenKind::If,
            TokenKind::Not,
            TokenKind::Exists,
            TokenKind::Ident,
            TokenKind::Semicolon,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn test_full_create_database_with_owner() {
    assert_eq!(
        lex("CREATE DATABASE mydb OWNER alice;"),
        vec![
            TokenKind::Create,
            TokenKind::Database,
            TokenKind::Ident,
            TokenKind::Owner,
            TokenKind::Ident,
            TokenKind::Semicolon,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn test_full_create_database_with_encoding() {
    assert_eq!(
        lex("CREATE DATABASE mydb ENCODING 'UTF8';"),
        vec![
            TokenKind::Create,
            TokenKind::Database,
            TokenKind::Ident,
            TokenKind::Encoding,
            TokenKind::StringLit,
            TokenKind::Semicolon,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn test_full_create_database_with_connection_limit() {
    assert_eq!(
        lex("CREATE DATABASE mydb CONNECTION LIMIT 100;"),
        vec![
            TokenKind::Create,
            TokenKind::Database,
            TokenKind::Ident,
            TokenKind::Connection,
            TokenKind::Limit,
            TokenKind::IntLit(100),
            TokenKind::Semicolon,
            TokenKind::Eof,
        ]
    );
}

#[test]
fn test_full_create_database_all_options() {
    let tokens = lex(
        "CREATE DATABASE mydb OWNER alice ENCODING 'UTF8' LOCALE 'en_US' TABLESPACE myspace CONNECTION LIMIT 50;",
    );
    assert_eq!(
        tokens,
        vec![
            TokenKind::Create,
            TokenKind::Database,
            TokenKind::Ident, // mydb
            TokenKind::Owner,
            TokenKind::Ident, // alice
            TokenKind::Encoding,
            TokenKind::StringLit, // 'UTF8'
            TokenKind::Locale,
            TokenKind::StringLit, // 'en_US'
            TokenKind::Tablespace,
            TokenKind::Ident, // myspace
            TokenKind::Connection,
            TokenKind::Limit,
            TokenKind::IntLit(50),
            TokenKind::Semicolon,
            TokenKind::Eof,
        ]
    );
}

// ── Spans ─────────────────────────────────────────────────────────────────────

#[test]
fn test_database_name_span() {
    let input = "CREATE DATABASE mydb;";
    let tokens: Vec<_> = Lexer::new(input).collect();
    let name_token = &tokens[2]; // mydb
    assert_eq!(&input[name_token.span.start..name_token.span.end], "mydb");
}

#[test]
fn test_line_and_column_tracking() {
    let input = "CREATE\nDATABASE\nmydb;";
    let tokens: Vec<_> = Lexer::new(input).collect();
    assert_eq!(tokens[0].span.line, 1); // CREATE
    assert_eq!(tokens[1].span.line, 2); // DATABASE
    assert_eq!(tokens[2].span.line, 3); // mydb
}
