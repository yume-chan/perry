//! Loop body purity analysis for issue #74.
//!
//! Detects loop bodies that have no LLVM-visible observable side effect.
//! Such bodies trigger clang -O3's loop-deletion / IndVarSimplify passes
//! to fold the loop to its closed-form result, which means a tight
//! `for (let i=0; i<N; i++) sum+=1;` between two `Date.now()` calls
//! would report 0ms wall-clock — confusingly making `Date.now()` look
//! broken when in fact the loop never ran.
//!
//! When [`body_is_observably_side_effect_free`] returns true, `lower_for`
//! / `lower_while` / `lower_do_while` insert an empty `asm sideeffect`
//! barrier in the body. The barrier is opaque to the optimizer (it
//! cannot prove the asm has no effect) so the loop is preserved
//! end-to-end, and emits zero machine instructions.
//!
//! The whitelist is intentionally narrow: anything that could throw,
//! call, allocate, mutate the heap, or yield to async machinery is
//! treated as a side effect. This means real workloads (array writes,
//! method calls, property mutations) are unaffected — vectorization
//! and LICM still apply because we don't insert the barrier there.

use perry_hir::{Expr, Stmt};

/// True when every statement in `body` is provably free of any
/// LLVM-visible side effect (no calls, no heap mutation, no throws,
/// no yields, no nested non-pure constructs).
pub(crate) fn body_is_observably_side_effect_free(body: &[Stmt]) -> bool {
    body.iter().all(stmt_is_pure)
}

fn stmt_is_pure(s: &Stmt) -> bool {
    match s {
        Stmt::Expr(e) => expr_is_pure(e),
        Stmt::Let { init, .. } => init.as_ref().map_or(true, expr_is_pure),
        Stmt::Return(_) | Stmt::Throw(_) => false,
        Stmt::If { condition, then_branch, else_branch, .. } => {
            expr_is_pure(condition)
                && then_branch.iter().all(stmt_is_pure)
                && else_branch.as_ref().map_or(true, |b| b.iter().all(stmt_is_pure))
        }
        // Nested loops: their own lowering applies the same analysis,
        // so reporting the outer body as pure when the inner is pure
        // is consistent (the inner loop will also get its barrier).
        Stmt::While { condition, body, .. } => {
            expr_is_pure(condition) && body.iter().all(stmt_is_pure)
        }
        Stmt::DoWhile { body, condition, .. } => {
            expr_is_pure(condition) && body.iter().all(stmt_is_pure)
        }
        Stmt::For { init, condition, update, body, .. } => {
            init.as_deref().map_or(true, stmt_is_pure)
                && condition.as_ref().map_or(true, expr_is_pure)
                && update.as_ref().map_or(true, expr_is_pure)
                && body.iter().all(stmt_is_pure)
        }
        Stmt::Labeled { body, .. } => stmt_is_pure(body),
        // Break/Continue are control flow; they don't add side effects
        // but they also mean the body's analysis has to assume the
        // surrounding loop's structure may not run linearly. Safe to
        // treat as pure — a loop whose body only does break/continue
        // and pure ops is still observably empty.
        Stmt::Break | Stmt::Continue
        | Stmt::LabeledBreak(_) | Stmt::LabeledContinue(_) => true,
        // Conservative for everything else (Try with catch can run
        // arbitrary code; Switch can have any case body).
        _ => false,
    }
}

fn expr_is_pure(e: &Expr) -> bool {
    match e {
        // Literals and pure reads.
        Expr::Undefined | Expr::Null | Expr::Bool(_) | Expr::Number(_)
        | Expr::Integer(_) | Expr::BigInt(_) | Expr::String(_)
        | Expr::This | Expr::LocalGet(_) | Expr::GlobalGet(_)
        | Expr::FuncRef(_) | Expr::ClassRef(_) | Expr::EnumMember { .. } => true,

        // Local mutations are pure at the LLVM level (alloca-promoted).
        // GlobalSet writes to a module global and IS observable.
        Expr::LocalSet(_, val) => expr_is_pure(val),

        // HIR's Update variant only ever targets a local (`id: LocalId`),
        // so it is always pure at the LLVM level. PropertyUpdate /
        // IndexUpdate live in their own variants and fall through to
        // the catch-all below.
        Expr::Update { .. } => true,

        // Pure arithmetic / logical / comparison ops.
        Expr::Binary { left, right, .. } => expr_is_pure(left) && expr_is_pure(right),
        Expr::Unary { operand, .. } => expr_is_pure(operand),
        Expr::Compare { left, right, .. } => expr_is_pure(left) && expr_is_pure(right),
        Expr::Logical { left, right, .. } => expr_is_pure(left) && expr_is_pure(right),
        Expr::Conditional { condition, then_expr, else_expr } => {
            expr_is_pure(condition) && expr_is_pure(then_expr) && expr_is_pure(else_expr)
        }
        Expr::TypeOf(operand) => expr_is_pure(operand),
        Expr::Void(operand) => expr_is_pure(operand),

        // Anything that calls a function, allocates, mutates the heap,
        // throws, or interacts with the runtime is conservatively a
        // side effect. The catch-all matters most: if a future HIR
        // variant escapes here, we'd rather miss the optimization than
        // wrongly insert a barrier and surprise the user.
        _ => false,
    }
}
