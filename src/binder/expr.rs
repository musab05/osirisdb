use std::time;

use crate::{
    ast::{BinOpKind, DataType, Expr, UnaryOpKind, Value},
    binder::BindError,
    common::{interner::Interner, symbol::Symbol},
};

/// Evaluates an [`Expr`] to a scalar [`Value`].
///
/// Does not handle column references, subqueries, or CASE — those need
/// row context or a full query executor that does not exist yet.
pub fn eval_expr(expr: &Expr, interner: &Interner) -> Result<Value, BindError> {
    match expr {
        Expr::Literal(v) => Ok(v.clone()),

        Expr::UnaryOp { op, expr } => {
            let val = eval_expr(expr, interner)?;
            eval_unary(op, val)
        }

        Expr::BinOp { op, lhs, rhs } => {
            let lhs_val = eval_expr(lhs, interner)?;
            let rhs_val = eval_expr(rhs, interner)?;
            eval_binop(op, lhs_val, rhs_val, interner)
        }

        Expr::IsNull { expr, negated } => {
            let val = eval_expr(expr, interner)?;
            let is_null = matches!(val, Value::Null);
            Ok(Value::Boolean(if *negated { !is_null } else { is_null }))
        }

        Expr::Between {
            expr,
            low,
            high,
            negated,
        } => {
            let val = eval_expr(expr, interner)?;
            let low_val = eval_expr(low, interner)?;
            let high_val = eval_expr(high, interner)?;

            let above_low = eval_binop(&BinOpKind::Ge, val.clone(), low_val, interner)?;
            let below_high = eval_binop(&BinOpKind::Le, val, high_val, interner)?;
            let in_range = eval_binop(&BinOpKind::And, above_low, below_high, interner)?;

            match (negated, in_range) {
                (true, Value::Boolean(b)) => Ok(Value::Boolean(!b)),
                (false, v) => Ok(v),
                _ => Ok(Value::Null),
            }
        }

        Expr::InList {
            expr,
            list,
            negated,
        } => {
            let val = eval_expr(expr, interner)?;
            if matches!(val, Value::Null) {
                return Ok(Value::Null);
            }
            let mut found = false;
            for item in list {
                let item_val = eval_expr(item, interner)?;
                if item_val == val {
                    found = true;
                    break;
                }
            }
            Ok(Value::Boolean(if *negated { !found } else { found }))
        }

        Expr::Cast { expr, ty } => {
            let val = eval_expr(expr, interner)?;
            eval_cast(val, ty, interner)
        }

        Expr::FuncCall { name, args } => eval_funcall(*name, args, interner),
        Expr::Case {
            operand,
            when_thens,
            else_,
        } => eval_case(operand, when_thens, else_, interner),

        Expr::Column { .. }
        | Expr::InSubquery { .. }
        | Expr::Exists { .. }
        | Expr::Subquery(_)
        | Expr::Wildcard => Err(BindError::UnsupportedExpression),
    }
}

// Unary operators

fn eval_unary(op: &UnaryOpKind, val: Value) -> Result<Value, BindError> {
    match (op, val) {
        (UnaryOpKind::Plus, Value::Int(n)) => Ok(Value::Int(n)),
        (UnaryOpKind::Plus, Value::Float(f)) => Ok(Value::Float(f)),
        (UnaryOpKind::Minus, Value::Int(n)) => Ok(Value::Int(-n)),
        (UnaryOpKind::Minus, Value::Float(f)) => Ok(Value::Float(-f)),
        (UnaryOpKind::Not, Value::Boolean(b)) => Ok(Value::Boolean(!b)),
        (UnaryOpKind::Not, Value::Null) => Ok(Value::Null),
        (_, Value::Null) => Ok(Value::Null),
        _ => Err(BindError::UnsupportedExpression),
    }
}

// Binary operators

fn eval_binop(
    op: &BinOpKind,
    lhs: Value,
    rhs: Value,
    interner: &Interner,
) -> Result<Value, BindError> {
    // NULL propagates through all binary operators per SQL standard.
    if matches!((&lhs, &rhs), (Value::Null, _) | (_, Value::Null)) {
        return Ok(Value::Null);
    }

    match (op, lhs, rhs) {
        // ── Arithmetic: Int op Int ────────────────────────────────────────
        (BinOpKind::Add, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a + b)),
        (BinOpKind::Sub, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a - b)),
        (BinOpKind::Mul, Value::Int(a), Value::Int(b)) => Ok(Value::Int(a * b)),
        (BinOpKind::Div, Value::Int(a), Value::Int(b)) => {
            if b == 0 {
                Err(BindError::DivisionByZero)
            } else {
                Ok(Value::Int(a / b))
            }
        }
        (BinOpKind::Mod, Value::Int(a), Value::Int(b)) => {
            if b == 0 {
                Err(BindError::DivisionByZero)
            } else {
                Ok(Value::Int(a % b))
            }
        }

        // ── Arithmetic: Float op Float ────────────────────────────────────
        (BinOpKind::Add, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a + b)),
        (BinOpKind::Sub, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a - b)),
        (BinOpKind::Mul, Value::Float(a), Value::Float(b)) => Ok(Value::Float(a * b)),
        (BinOpKind::Div, Value::Float(a), Value::Float(b)) => {
            if b == 0.0 {
                Err(BindError::DivisionByZero)
            } else {
                Ok(Value::Float(a / b))
            }
        }

        // ── Arithmetic: mixed Int + Float (widening) ──────────────────────
        (BinOpKind::Add, Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 + b)),
        (BinOpKind::Add, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a + b as f64)),
        (BinOpKind::Sub, Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 - b)),
        (BinOpKind::Sub, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a - b as f64)),
        (BinOpKind::Mul, Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 * b)),
        (BinOpKind::Mul, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a * b as f64)),
        (BinOpKind::Div, Value::Int(a), Value::Float(b)) => Ok(Value::Float(a as f64 / b)),
        (BinOpKind::Div, Value::Float(a), Value::Int(b)) => Ok(Value::Float(a / b as f64)),

        // ── String concatenation ──────────────────────────────────────────
        (BinOpKind::Add, Value::String(a), Value::String(b)) => {
            let s = format!("{}{}", interner.resolve(a), interner.resolve(b));
            let sym = interner.intern(&s);
            Ok(Value::String(sym))
        }

        // ── Comparison: Int ───────────────────────────────────────────────
        (BinOpKind::Eq, Value::Int(a), Value::Int(b)) => Ok(Value::Boolean(a == b)),
        (BinOpKind::Ne, Value::Int(a), Value::Int(b)) => Ok(Value::Boolean(a != b)),
        (BinOpKind::Lt, Value::Int(a), Value::Int(b)) => Ok(Value::Boolean(a < b)),
        (BinOpKind::Le, Value::Int(a), Value::Int(b)) => Ok(Value::Boolean(a <= b)),
        (BinOpKind::Gt, Value::Int(a), Value::Int(b)) => Ok(Value::Boolean(a > b)),
        (BinOpKind::Ge, Value::Int(a), Value::Int(b)) => Ok(Value::Boolean(a >= b)),

        // ── Comparison: Float ─────────────────────────────────────────────
        (BinOpKind::Eq, Value::Float(a), Value::Float(b)) => Ok(Value::Boolean(a == b)),
        (BinOpKind::Ne, Value::Float(a), Value::Float(b)) => Ok(Value::Boolean(a != b)),
        (BinOpKind::Lt, Value::Float(a), Value::Float(b)) => Ok(Value::Boolean(a < b)),
        (BinOpKind::Le, Value::Float(a), Value::Float(b)) => Ok(Value::Boolean(a <= b)),
        (BinOpKind::Gt, Value::Float(a), Value::Float(b)) => Ok(Value::Boolean(a > b)),
        (BinOpKind::Ge, Value::Float(a), Value::Float(b)) => Ok(Value::Boolean(a >= b)),

        // ── Comparison: mixed Int/Float ───────────────────────────────────
        (BinOpKind::Eq, Value::Int(a), Value::Float(b)) => Ok(Value::Boolean(a as f64 == b)),
        (BinOpKind::Ne, Value::Int(a), Value::Float(b)) => Ok(Value::Boolean(a as f64 != b)),
        (BinOpKind::Lt, Value::Int(a), Value::Float(b)) => Ok(Value::Boolean((a as f64) < b)),
        (BinOpKind::Le, Value::Int(a), Value::Float(b)) => Ok(Value::Boolean(a as f64 <= b)),
        (BinOpKind::Gt, Value::Int(a), Value::Float(b)) => Ok(Value::Boolean(a as f64 > b)),
        (BinOpKind::Ge, Value::Int(a), Value::Float(b)) => Ok(Value::Boolean(a as f64 >= b)),

        // ── Comparison: String ────────────────────────────────────────────
        (BinOpKind::Eq, Value::String(a), Value::String(b)) => Ok(Value::Boolean(a == b)),
        (BinOpKind::Ne, Value::String(a), Value::String(b)) => Ok(Value::Boolean(a != b)),

        // ── Comparison: Boolean ───────────────────────────────────────────
        (BinOpKind::Eq, Value::Boolean(a), Value::Boolean(b)) => Ok(Value::Boolean(a == b)),
        (BinOpKind::Ne, Value::Boolean(a), Value::Boolean(b)) => Ok(Value::Boolean(a != b)),

        // ── Logical ───────────────────────────────────────────────────────
        (BinOpKind::And, Value::Boolean(a), Value::Boolean(b)) => Ok(Value::Boolean(a && b)),
        (BinOpKind::Or, Value::Boolean(a), Value::Boolean(b)) => Ok(Value::Boolean(a || b)),

        // ── Like, In, Between — handled at Expr level, never reach here ───
        (BinOpKind::Like, ..) | (BinOpKind::In, ..) | (BinOpKind::Between, ..) => {
            Err(BindError::UnsupportedExpression)
        }

        _ => Err(BindError::UnsupportedExpression),
    }
}

// CAST

fn eval_cast(val: Value, ty: &DataType, interner: &Interner) -> Result<Value, BindError> {
    match (val, ty) {
        (Value::Null, _) => Ok(Value::Null),

        // ── Int → numeric ─────────────────────────────────────────────────
        (Value::Int(n), DataType::Float | DataType::Double) => Ok(Value::Float(n as f64)),
        (Value::Int(n), DataType::BigInt | DataType::Int) => Ok(Value::Int(n)),
        (Value::Int(n), DataType::SmallInt) => {
            if n >= i16::MIN as i64 && n <= i16::MAX as i64 {
                Ok(Value::Int(n))
            } else {
                Err(BindError::TypeMismatch {
                    // No column context in a CAST — use a sentinel symbol.
                    col: Symbol(0),
                    row: 0,
                    expected: "SMALLINT (-32768..32767)",
                    got: "integer out of range",
                })
            }
        }

        // ── Float → numeric ───────────────────────────────────────────────
        (Value::Float(f), DataType::Int | DataType::BigInt) => Ok(Value::Int(f as i64)),
        (Value::Float(f), DataType::Float | DataType::Double) => Ok(Value::Float(f)),

        // ── Numeric/Bool → String ─────────────────────────────────────────
        (Value::Int(n), DataType::VarChar(_) | DataType::Text | DataType::Char(_)) => {
            let sym = interner.intern(&n.to_string());
            Ok(Value::String(sym))
        }
        (Value::Float(f), DataType::VarChar(_) | DataType::Text | DataType::Char(_)) => {
            let sym = interner.intern(&f.to_string());
            Ok(Value::String(sym))
        }
        (Value::Boolean(b), DataType::VarChar(_) | DataType::Text | DataType::Char(_)) => {
            let sym = interner.intern(if b { "true" } else { "false" });
            Ok(Value::String(sym))
        }

        // ── Boolean → Boolean ─────────────────────────────────────────────
        (Value::Boolean(b), DataType::Boolean) => Ok(Value::Boolean(b)),

        _ => Err(BindError::UnsupportedExpression),
    }
}

// Function calls

fn eval_funcall(name: Symbol, args: &[Expr], interner: &Interner) -> Result<Value, BindError> {
    let fn_name = interner.resolve(name).to_lowercase();

    match fn_name.as_str() {
        // ── Timestamp stubs — real impl needs system clock ────────────────
        "now" | "current_timestamp" => {
            let duration = time::SystemTime::now()
                .duration_since(time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            let secs = duration.as_secs();
            let (y, m, d, hh, mm, ss) = seconds_to_datetime(secs);

            let timestamp_str = format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, hh, mm, ss);

            let sym = interner.intern(&timestamp_str);
            Ok(Value::String(sym))
        }
        "current_date" => {
            let duration = time::SystemTime::now()
                .duration_since(time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            let secs = duration.as_secs();
            let (y, m, d, _, _, _) = seconds_to_datetime(secs);

            // Format as YYYY-MM-DD
            let date_str = format!("{:04}-{:02}-{:02}", y, m, d);
            let sym = interner.intern(&date_str);
            Ok(Value::String(sym))
        }

        // ── String functions ──────────────────────────────────────────────
        "upper" => {
            if args.len() != 1 {
                return Err(BindError::UnsupportedExpression);
            }
            match eval_expr(&args[0], interner)? {
                Value::String(sym) => {
                    let upper = interner.resolve(sym).to_uppercase();
                    let new_sym = interner.intern(&upper);
                    Ok(Value::String(new_sym))
                }
                Value::Null => Ok(Value::Null),
                _ => Err(BindError::UnsupportedExpression),
            }
        }
        "lower" => {
            if args.len() != 1 {
                return Err(BindError::UnsupportedExpression);
            }
            match eval_expr(&args[0], interner)? {
                Value::String(sym) => {
                    let lower = interner.resolve(sym).to_lowercase();
                    let new_sym = interner.intern(&lower);
                    Ok(Value::String(new_sym))
                }
                Value::Null => Ok(Value::Null),
                _ => Err(BindError::UnsupportedExpression),
            }
        }

        // ── COALESCE — return first non-NULL arg ──────────────────────────
        "coalesce" => {
            for arg in args {
                let val = eval_expr(arg, interner)?;
                if !matches!(val, Value::Null) {
                    return Ok(val);
                }
            }
            Ok(Value::Null)
        }

        _ => Err(BindError::UnsupportedExpression),
    }
}

/// Helper to convert UNIX epoch seconds to (Year, Month, Day, Hour, Minute, Second) UTC.
fn seconds_to_datetime(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let sec = (secs % 60) as u32;
    let mins = secs / 60;
    let min = (mins % 60) as u32;
    let hours = mins / 60;
    let hour = (hours % 24) as u32;
    let days = hour / 24;

    // UNIX epoch starts at 1970-01-01
    let mut year = 1970;
    let mut days_rem = days;
    loop {
        let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
        let days_in_year = if is_leap { 366 } else { 365 };
        if days_rem < days_in_year {
            break;
        }

        days_rem -= days_in_year;
        year += 1;
    }

    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
    let month_lengths = if is_leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 0;
    while days_rem >= month_lengths[month] {
        days_rem -= month_lengths[month];
        month += 1;
    }

    let month = (month + 1) as u32;
    let day = (days_rem + 1) as u32;
    (year, month, day, hour, min, sec)
}

fn eval_case(
    operand: &Option<Box<Expr>>,
    when_thens: &Vec<(Expr, Expr)>,
    else_: &Option<Box<Expr>>,
    interner: &Interner,
) -> Result<Value, BindError> {
    // If a search operand is provided, evaluate it first
    let op_val = match operand {
        Some(op_expr) => Some(eval_expr(op_expr, interner)?),
        None => None,
    };

    let mut matched_then = None;

    // Iterate through each WHEN-THEN branch
    for (cond_expr, then_expr) in when_thens {
        match &op_val {
            Some(val) => {
                // Simple CASE: Compare operand value directly with branch condition value
                let cond_val = eval_expr(cond_expr, interner)?;
                if val == &cond_val {
                    matched_then = Some(then_expr);
                    break;
                }
            }
            None => {
                // Searched CASE: Branch condition must evaluate to a boolean
                let cond_val = eval_expr(cond_expr, interner)?;
                if let Value::Boolean(true) = cond_val {
                    matched_then = Some(then_expr);
                    break;
                }
            }
        }
    }

    // Return the evaluated result of the matched branch, fallback to ELSE, or return NULL
    match matched_then {
        Some(then_expr) => eval_expr(then_expr, interner),
        None => match else_ {
            Some(else_expr) => eval_expr(else_expr, interner),
            None => Ok(Value::Null), // Standard SQL returns NULL if no branch matches and there is no ELSE
        },
    }
}
