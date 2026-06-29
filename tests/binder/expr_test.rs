use osirisdb::ast::{Expr, Value};
use osirisdb::binder::expr::eval_expr;
use osirisdb::common::interner::Interner;

fn make_literal(v: Value) -> Expr {
    Expr::Literal(v)
}

#[test]
fn test_case_searched() {
    let interner = Interner::new();

    // CASE WHEN true THEN 42 ELSE 100 END -> should return 42
    let expr = Expr::Case {
        operand: None,
        when_thens: vec![(
            make_literal(Value::Boolean(true)),
            make_literal(Value::Int(42)),
        )],
        else_: Some(Box::new(make_literal(Value::Int(100)))),
    };
    assert_eq!(eval_expr(&expr, &interner), Ok(Value::Int(42)));

    // CASE WHEN false THEN 42 ELSE 100 END -> should return 100
    let expr = Expr::Case {
        operand: None,
        when_thens: vec![(
            make_literal(Value::Boolean(false)),
            make_literal(Value::Int(42)),
        )],
        else_: Some(Box::new(make_literal(Value::Int(100)))),
    };
    assert_eq!(eval_expr(&expr, &interner), Ok(Value::Int(100)));

    // CASE WHEN false THEN 42 END -> should return NULL
    let expr = Expr::Case {
        operand: None,
        when_thens: vec![(
            make_literal(Value::Boolean(false)),
            make_literal(Value::Int(42)),
        )],
        else_: None,
    };
    assert_eq!(eval_expr(&expr, &interner), Ok(Value::Null));
}

#[test]
fn test_case_simple() {
    let interner = Interner::new();

    // CASE 5 WHEN 5 THEN 42 ELSE 100 END -> should return 42
    let expr = Expr::Case {
        operand: Some(Box::new(make_literal(Value::Int(5)))),
        when_thens: vec![(make_literal(Value::Int(5)), make_literal(Value::Int(42)))],
        else_: Some(Box::new(make_literal(Value::Int(100)))),
    };
    assert_eq!(eval_expr(&expr, &interner), Ok(Value::Int(42)));

    // CASE 5 WHEN 10 THEN 42 ELSE 100 END -> should return 100
    let expr = Expr::Case {
        operand: Some(Box::new(make_literal(Value::Int(5)))),
        when_thens: vec![(make_literal(Value::Int(10)), make_literal(Value::Int(42)))],
        else_: Some(Box::new(make_literal(Value::Int(100)))),
    };
    assert_eq!(eval_expr(&expr, &interner), Ok(Value::Int(100)));
}
