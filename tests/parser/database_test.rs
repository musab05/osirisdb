use osirisdb::ast::Statement;
use osirisdb::parser::parser::Parser;

fn parse(sql: &str) -> Vec<Statement> {
    Parser::new(sql).parse().expect("expected successful parse")
}

fn parse_err(sql: &str) -> String {
    Parser::new(sql).parse().unwrap_err().message
}

// ── Minimal ───────────────────────────────────────────────────────────────────

#[test]
fn test_minimal() {
    let stmts = parse("CREATE DATABASE mydb;");
    assert_eq!(stmts.len(), 1);
    assert!(matches!(stmts[0], Statement::CreateDatabase(_)));
}

#[test]
fn test_without_semicolon() {
    let stmts = parse("CREATE DATABASE mydb");
    assert_eq!(stmts.len(), 1);
}

// ── IF NOT EXISTS ─────────────────────────────────────────────────────────────

#[test]
fn test_if_not_exists_true() {
    let stmts = parse("CREATE DATABASE IF NOT EXISTS mydb;");
    match &stmts[0] {
        Statement::CreateDatabase(s) => assert!(s.if_not_exists),
        _ => panic!("expected CreateDatabase"),
    }
}

#[test]
fn test_if_not_exists_false_when_absent() {
    let stmts = parse("CREATE DATABASE mydb;");
    match &stmts[0] {
        Statement::CreateDatabase(s) => assert!(!s.if_not_exists),
        _ => panic!("expected CreateDatabase"),
    }
}

// ── OWNER ─────────────────────────────────────────────────────────────────────

#[test]
fn test_owner_present() {
    let stmts = parse("CREATE DATABASE mydb OWNER alice;");
    match &stmts[0] {
        Statement::CreateDatabase(s) => assert!(s.owner.is_some()),
        _ => panic!("expected CreateDatabase"),
    }
}

#[test]
fn test_owner_absent() {
    let stmts = parse("CREATE DATABASE mydb;");
    match &stmts[0] {
        Statement::CreateDatabase(s) => assert!(s.owner.is_none()),
        _ => panic!("expected CreateDatabase"),
    }
}

#[test]
fn test_owner_with_eq() {
    // OWNER = alice is valid too
    let stmts = parse("CREATE DATABASE mydb OWNER = alice;");
    match &stmts[0] {
        Statement::CreateDatabase(s) => assert!(s.owner.is_some()),
        _ => panic!("expected CreateDatabase"),
    }
}

// ── ENCODING ──────────────────────────────────────────────────────────────────

#[test]
fn test_encoding_present() {
    let stmts = parse("CREATE DATABASE mydb ENCODING 'UTF8';");
    match &stmts[0] {
        Statement::CreateDatabase(s) => assert!(s.encoding.is_some()),
        _ => panic!("expected CreateDatabase"),
    }
}

#[test]
fn test_encoding_with_eq() {
    let stmts = parse("CREATE DATABASE mydb ENCODING = 'UTF8';");
    match &stmts[0] {
        Statement::CreateDatabase(s) => assert!(s.encoding.is_some()),
        _ => panic!("expected CreateDatabase"),
    }
}

// ── LOCALE ────────────────────────────────────────────────────────────────────

#[test]
fn test_locale_present() {
    let stmts = parse("CREATE DATABASE mydb LOCALE 'en_US';");
    match &stmts[0] {
        Statement::CreateDatabase(s) => assert!(s.locale.is_some()),
        _ => panic!("expected CreateDatabase"),
    }
}

// ── TABLESPACE ────────────────────────────────────────────────────────────────

#[test]
fn test_tablespace_present() {
    let stmts = parse("CREATE DATABASE mydb TABLESPACE myspace;");
    match &stmts[0] {
        Statement::CreateDatabase(s) => assert!(s.tablespace.is_some()),
        _ => panic!("expected CreateDatabase"),
    }
}

// ── CONNECTION LIMIT ──────────────────────────────────────────────────────────

#[test]
fn test_connection_limit() {
    let stmts = parse("CREATE DATABASE mydb CONNECTION LIMIT 100;");
    match &stmts[0] {
        Statement::CreateDatabase(s) => assert_eq!(s.connection_limit, Some(100)),
        _ => panic!("expected CreateDatabase"),
    }
}

#[test]
fn test_connection_limit_negative_one() {
    // -1 means unlimited
    let stmts = parse("CREATE DATABASE mydb CONNECTION LIMIT -1;");
    match &stmts[0] {
        Statement::CreateDatabase(s) => assert_eq!(s.connection_limit, Some(-1)),
        _ => panic!("expected CreateDatabase"),
    }
}

#[test]
fn test_connection_limit_with_eq() {
    let stmts = parse("CREATE DATABASE mydb CONNECTION LIMIT = 50;");
    match &stmts[0] {
        Statement::CreateDatabase(s) => assert_eq!(s.connection_limit, Some(50)),
        _ => panic!("expected CreateDatabase"),
    }
}

// ── Full statement ────────────────────────────────────────────────────────────

#[test]
fn test_all_options() {
    let stmts = parse(
        "CREATE DATABASE mydb OWNER alice ENCODING 'UTF8' LOCALE 'en_US' TABLESPACE myspace CONNECTION LIMIT 50;",
    );
    match &stmts[0] {
        Statement::CreateDatabase(s) => {
            assert!(!s.if_not_exists);
            assert!(s.owner.is_some());
            assert!(s.encoding.is_some());
            assert!(s.locale.is_some());
            assert!(s.tablespace.is_some());
            assert_eq!(s.connection_limit, Some(50));
        }
        _ => panic!("expected CreateDatabase"),
    }
}

#[test]
fn test_use_database() {
    let mut parser = Parser::new("USE mydb;");
    let stmts = parser.parse().expect("expected successful parse");
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        Statement::UseDatabase(s) => {
            assert_eq!(parser.interner.resolve(s.database_name), "mydb");
        }
        _ => panic!("expected UseDatabase"),
    }
}

#[test]
fn test_options_any_order() {
    // options can appear in any order
    let a = parse("CREATE DATABASE mydb OWNER alice ENCODING 'UTF8';");
    let b = parse("CREATE DATABASE mydb ENCODING 'UTF8' OWNER alice;");
    assert!(matches!(a[0], Statement::CreateDatabase(_)));
    assert!(matches!(b[0], Statement::CreateDatabase(_)));
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[test]
fn test_missing_name() {
    let err = parse_err("CREATE DATABASE;");
    assert!(!err.is_empty());
}

#[test]
fn test_missing_name_with_if_not_exists() {
    let err = parse_err("CREATE DATABASE IF NOT EXISTS;");
    assert!(!err.is_empty());
}
