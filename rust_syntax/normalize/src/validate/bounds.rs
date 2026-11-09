//! Structural bounds shared by object counts and mode cardinality.
//! Dynamic expressions retain their execution-time amount/availability rules;
//! validation establishes their references, not a guessed ordering of values.
use crate::ir::Expr;

pub(super) fn nonnegative(value: &Expr) -> Result<(), String> {
    if matches!(value, Expr::Constant(n) if *n < 0) {
        return Err("negative selection count".into());
    }
    Ok(())
}

pub(super) fn interval(minimum: &Expr, maximum: &Expr) -> Result<(), String> {
    nonnegative(minimum)?;
    nonnegative(maximum)?;
    if matches!((minimum, maximum), (Expr::Constant(min), Expr::Constant(max)) if min > max) {
        return Err("selection lower bound exceeds upper bound".into());
    }
    Ok(())
}
