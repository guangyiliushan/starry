//! Structured definite-assignment analysis.
//!
//! Tracks `DefKind::LocalBinding` only. Top-level globals are predeclared before
//! function bodies: a global with an initializer is initialized before `main`,
//! regardless of source order. `block`/`stmt` return whether control definitely
//! leaves the current block; branch merges ignore exiting arms.

use std::collections::HashSet;

use ast::syntax::decl::{BindingDecl, DeclKind, FunctionBody};
use ast::syntax::expr::{Block, Condition, ExprKind, ExprWrap, WhenPattern};
use ast::syntax::node::{DiagCode, Diagnostic, SourceFile};
use ast::syntax::pat::{PatKind, PatWrap};
use ast::syntax::stmt::{StmtKind, StmtWrap};
use ast::token::OperatorKind;

use crate::SemState;
use crate::def::{DefId, DefKind};

pub(crate) fn run(ast: &SourceFile, st: &mut SemState) {
    let mut init = HashSet::new();
    // Parameters are initialized on entry. Globals with initializers are also
    // initialized before any called function can observe them.
    for id in 0..st.defs.len() {
        let def = DefId(id as u32);
        if matches!(st.defs.get(def).kind, DefKind::Param) {
            init.insert(def);
        }
    }
    for item in &ast.items {
        if let DeclKind::Binding(binding) = &item.kind {
            declare_pattern_st(st, &binding.pattern, binding.init.is_some(), &mut init);
        }
    }

    for item in &ast.items {
        match &item.kind {
            DeclKind::Fun(f) => {
                if let Some(body) = &f.body {
                    match body {
                        FunctionBody::Block(b) => {
                            let _ = block(b, &mut init, st);
                        }
                        FunctionBody::Expr(e) => expr(e, &mut init, st),
                    }
                }
            }
            DeclKind::Binding(b) => {
                if let Some(value) = &b.init {
                    expr(value, &mut init, st);
                    declare_pattern_st(st, &b.pattern, true, &mut init);
                }
            }
            _ => {}
        }
    }
}

fn declare_pattern_st(st: &SemState, pat: &PatWrap, initialized: bool, init: &mut HashSet<DefId>) {
    if let PatKind::Binding { .. } = &pat.kind
        && let Some(&def) = st.pat_binding_def.get(pat.id)
        && matches!(st.defs.get(def).kind, DefKind::LocalBinding { .. })
    {
        if initialized {
            init.insert(def);
        } else {
            init.remove(&def);
        }
    }
    match &pat.kind {
        PatKind::Or(ps)
        | PatKind::Tuple { elements: ps, .. }
        | PatKind::Array { elements: ps, .. } => {
            for p in ps {
                declare_pattern_st(st, p, initialized, init);
            }
        }
        PatKind::Constructor { args, .. } => {
            for p in args {
                declare_pattern_st(st, p, initialized, init);
            }
        }
        PatKind::Struct { fields, .. } => {
            for (_, p) in fields {
                declare_pattern_st(st, p, initialized, init);
            }
        }
        _ => {}
    }
}

fn initialized_after_branches(lhs: HashSet<DefId>, rhs: HashSet<DefId>) -> HashSet<DefId> {
    lhs.intersection(&rhs).copied().collect()
}

/// Returns true when control definitely leaves this block before its tail.
fn block(b: &Block, init: &mut HashSet<DefId>, st: &mut SemState) -> bool {
    for s in &b.stmts {
        if stmt(s, init, st) {
            return true;
        }
    }
    if let Some(t) = &b.tail {
        expr(t, init, st);
    }
    false
}

/// Returns true for statements that definitely exit the current block.
fn stmt(s: &StmtWrap, init: &mut HashSet<DefId>, st: &mut SemState) -> bool {
    match &s.kind {
        StmtKind::Expr(es) => {
            expr(&es.expr, init, st);
            false
        }
        StmtKind::Assignment { target, op, value } => {
            if *op == OperatorKind::Eq {
                expr(value, init, st);
                if matches!(target.kind, ExprKind::Ident(_)) {
                    assign_target_st(target, init, st);
                } else {
                    expr(target, init, st);
                }
            } else {
                expr(target, init, st);
                expr(value, init, st);
                if matches!(target.kind, ExprKind::Ident(_)) {
                    assign_target_st(target, init, st);
                }
            }
            false
        }
        StmtKind::Binding(b) => {
            binding(b, init, st);
            false
        }
        StmtKind::For {
            pattern,
            iterable,
            body,
            ..
        } => {
            expr(iterable, init, st);
            declare_pattern_st(st, pattern, true, init);
            let mut body_init = init.clone();
            let _ = block(body, &mut body_init, st);
            // A for loop may execute zero times: the exit state stays unchanged.
            false
        }
        StmtKind::While { cond, body, .. } => {
            condition(cond, init, st);
            let mut body_init = init.clone();
            let _ = block(body, &mut body_init, st);
            false
        }
        StmtKind::Loop { body, .. } => {
            let mut body_init = init.clone();
            let _ = block(body, &mut body_init, st);
            false
        }
        StmtKind::Break { value, .. } => {
            if let Some(v) = value {
                expr(v, init, st);
            }
            true
        }
        StmtKind::Continue { .. } => true,
        StmtKind::Return { value } => {
            if let Some(v) = value {
                expr(v, init, st);
            }
            true
        }
        StmtKind::Defer { body } => {
            let _ = block(body, init, st);
            false
        }
        StmtKind::Comptime { block: b } => {
            let _ = block(b, init, st);
            false
        }
        StmtKind::Grow { expr: e, bits } => {
            expr(e, init, st);
            if let Some(b) = bits {
                expr(b, init, st);
            }
            false
        }
        StmtKind::Error(_) | StmtKind::Missing(_) => false,
    }
}

fn binding(b: &BindingDecl, init: &mut HashSet<DefId>, st: &mut SemState) {
    if let Some(value) = &b.init {
        expr(value, init, st);
        declare_pattern_st(st, &b.pattern, true, init);
    } else {
        declare_pattern_st(st, &b.pattern, false, init);
    }
}

fn assign_target_st(target: &ExprWrap, init: &mut HashSet<DefId>, st: &SemState) {
    if let ExprKind::Ident(_) = &target.kind
        && let Some(&def) = st.expr_def.get(target.id)
        && matches!(st.defs.get(def).kind, DefKind::LocalBinding { .. })
    {
        init.insert(def);
    }
}

fn condition(c: &Condition, init: &mut HashSet<DefId>, st: &mut SemState) {
    match c {
        Condition::Expr(e) => expr(e, init, st),
        Condition::Val { pattern, expr: e } => {
            expr(e, init, st);
            declare_pattern_st(st, pattern, true, init);
        }
    }
}

fn check_ident(e: &ExprWrap, init: &HashSet<DefId>, st: &mut SemState) {
    if let Some(&def) = st.expr_def.get(e.id)
        && matches!(st.defs.get(def).kind, DefKind::LocalBinding { .. })
        && !init.contains(&def)
    {
        let name = st.defs.get(def).name;
        st.diags.emit(Diagnostic::error(
            DiagCode::SemUninitializedBinding,
            e.span,
            format!(
                "`{}` is read before it is definitely assigned",
                st.interner.resolve(name)
            ),
        ));
    }
}

fn expr(e: &ExprWrap, init: &mut HashSet<DefId>, st: &mut SemState) {
    match &e.kind {
        ExprKind::Literal(_) | ExprKind::Bool(_) | ExprKind::Null => {}
        ExprKind::Ident(_) | ExprKind::Path(_) => check_ident(e, init, st),
        ExprKind::Binary { lhs, rhs, .. } | ExprKind::In { lhs, rhs, .. } => {
            expr(lhs, init, st);
            expr(rhs, init, st);
        }
        ExprKind::Is { lhs, pattern, .. } => {
            expr(lhs, init, st);
            declare_pattern_st(st, pattern, true, init);
        }
        ExprKind::Unary { operand, .. }
        | ExprKind::ForceUnwrap { expr: operand }
        | ExprKind::Await { expr: operand }
        | ExprKind::Cast { expr: operand, .. }
        | ExprKind::ArrayRepeat { value: operand, .. } => expr(operand, init, st),
        ExprKind::Async { block: b } | ExprKind::Concurrent { block: b } => {
            let _ = block(b, init, st);
        }
        ExprKind::Loop { body } => {
            let mut body_init = init.clone();
            let _ = block(body, &mut body_init, st);
        }
        ExprKind::Unsafe { block: b } | ExprKind::Comptime { block: b } => {
            let _ = block(b, init, st);
        }
        ExprKind::Lambda { .. }
        | ExprKind::Spawn(_)
        | ExprKind::Proceed
        | ExprKind::Super { .. }
        | ExprKind::MacroCall { .. } => {}
        ExprKind::Call { callee, args, .. } => {
            expr(callee, init, st);
            for a in args {
                expr(&a.value, init, st);
            }
        }
        ExprKind::Index { base, index, .. } => {
            expr(base, init, st);
            expr(index, init, st);
        }
        ExprKind::Member { base, .. } => expr(base, init, st),
        ExprKind::Tuple(xs) | ExprKind::Array(xs) => {
            for x in xs {
                expr(x, init, st);
            }
        }
        ExprKind::StringInterp { parts } => {
            for p in parts {
                expr(p, init, st);
            }
        }
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => {
            condition(cond, init, st);
            let mut then_init = init.clone();
            let then_exits = block(then_block, &mut then_init, st);
            let (else_init, else_exits) = if let Some(eb) = else_block {
                let mut else_init = init.clone();
                let exits = block(eb, &mut else_init, st);
                (else_init, exits)
            } else {
                (init.clone(), false)
            };
            *init = match (then_exits, else_exits) {
                (false, false) => initialized_after_branches(then_init, else_init),
                (true, false) => else_init,
                (false, true) => then_init,
                // Both arms leave: the continuation is unreachable; keep a valid state.
                (true, true) => init.clone(),
            };
        }
        ExprKind::When { subject, branches } => {
            if let Some(s) = subject {
                expr(s, init, st);
            }
            let mut merged: Option<HashSet<DefId>> = None;
            let mut has_continuing_branch = false;
            for branch in branches {
                let mut branch_init = init.clone();
                if let WhenPattern::Pattern(p) = &branch.pattern {
                    declare_pattern_st(st, p, true, &mut branch_init);
                }
                if let Some(g) = &branch.guard {
                    expr(g, &mut branch_init, st);
                }
                let exits = match &branch.body {
                    ast::syntax::expr::WhenBody::Expr(e) => {
                        expr(e, &mut branch_init, st);
                        false
                    }
                    ast::syntax::expr::WhenBody::Block(b) => block(b, &mut branch_init, st),
                };
                if exits {
                    continue;
                }
                has_continuing_branch = true;
                merged = Some(match merged {
                    None => branch_init,
                    Some(prev) => initialized_after_branches(prev, branch_init),
                });
            }
            if has_continuing_branch && let Some(merged) = merged {
                *init = merged;
            }
        }
        ExprKind::Error(_) | ExprKind::Missing(_) => {}
    }
}
