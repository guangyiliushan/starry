//! 明确赋值分析（结构化保守版）。
//!
//! 只追踪 `DefKind::LocalBinding`：声明后置未初始化，简单赋值置已初始化；
//! if/when 取分支交集，循环按零次执行保守（循环内赋值不外溢到出口）。
//! 目标是拒绝 `var x: int; return x` 这类确定错误，不把复杂控制流
//! 推断成完整数据流求解器。

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
    // 形参在进入函数体前必然已初始化；全局收集即可（per-function 也成立）。
    for id in 0..st.defs.len() {
        let def = DefId(id as u32);
        if matches!(st.defs.get(def).kind, DefKind::Param) {
            init.insert(def);
        }
    }
    for item in &ast.items {
        match &item.kind {
            DeclKind::Fun(f) => {
                if let Some(body) = &f.body {
                    match body {
                        FunctionBody::Block(b) => block(b, &mut init, st),
                        FunctionBody::Expr(e) => expr(e, &mut init, st),
                    }
                }
            }
            DeclKind::Binding(b) => {
                if let Some(value) = &b.init {
                    expr(value, &mut init, st);
                    declare_pattern_st(st, &b.pattern, true, &mut init);
                } else {
                    declare_pattern_st(st, &b.pattern, false, &mut init);
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

fn block(b: &Block, init: &mut HashSet<DefId>, st: &mut SemState) {
    for s in &b.stmts {
        stmt(s, init, st);
    }
    if let Some(t) = &b.tail {
        expr(t, init, st);
    }
}

fn stmt(s: &StmtWrap, init: &mut HashSet<DefId>, st: &mut SemState) {
    match &s.kind {
        StmtKind::Expr(es) => expr(&es.expr, init, st),
        StmtKind::Assignment { target, op, value } => {
            if *op == OperatorKind::Eq {
                // 简单赋值 LHS 不是读取；先 RHS，后标记目标。
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
                } else {
                    expr(target, init, st);
                }
            }
        }
        StmtKind::Binding(b) => binding(b, init, st),
        StmtKind::For {
            pattern,
            iterable,
            body,
            ..
        } => {
            expr(iterable, init, st);
            declare_pattern_st(st, pattern, true, init);
            let mut body_init = init.clone();
            block(body, &mut body_init, st);
            // for 可零次执行：出口状态保持入口状态。
        }
        StmtKind::While { cond, body, .. } => {
            condition(cond, init, st);
            let mut body_init = init.clone();
            block(body, &mut body_init, st);
        }
        StmtKind::Loop { body, .. } => {
            let mut body_init = init.clone();
            block(body, &mut body_init, st);
        }
        StmtKind::Break { value, .. } => {
            if let Some(v) = value {
                expr(v, init, st);
            }
        }
        StmtKind::Continue { .. } => {}
        StmtKind::Return { value } => {
            if let Some(v) = value {
                expr(v, init, st);
            }
        }
        StmtKind::Defer { body } => block(body, init, st),
        StmtKind::Comptime { block: b } => block(b, init, st),
        StmtKind::Grow { expr: e, bits } => {
            expr(e, init, st);
            if let Some(b) = bits {
                expr(b, init, st);
            }
        }
        StmtKind::Error(_) | StmtKind::Missing(_) => {}
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

fn expr(e: &ExprWrap, init: &mut HashSet<DefId>, st: &mut SemState) {
    match &e.kind {
        ExprKind::Literal(_) | ExprKind::Bool(_) | ExprKind::Null => {}
        ExprKind::Ident(_) => {
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
        ExprKind::Path(_) => {
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
        ExprKind::Async { block: b } | ExprKind::Concurrent { block: b } => block(b, init, st),
        ExprKind::Loop { body } => {
            let mut body_init = init.clone();
            block(body, &mut body_init, st);
        }
        ExprKind::Unsafe { block: b } | ExprKind::Comptime { block: b } => block(b, init, st),
        ExprKind::Lambda { .. }
        | ExprKind::Spawn(_)
        | ExprKind::Proceed
        | ExprKind::Super { .. } => {}
        ExprKind::MacroCall { .. } => {}
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
            block(then_block, &mut then_init, st);
            let merged = if let Some(eb) = else_block {
                let mut else_init = init.clone();
                block(eb, &mut else_init, st);
                initialized_after_branches(then_init, else_init)
            } else {
                initialized_after_branches(then_init, init.clone())
            };
            *init = merged;
        }
        ExprKind::When { subject, branches } => {
            if let Some(s) = subject {
                expr(s, init, st);
            }
            let mut merged: Option<HashSet<DefId>> = None;
            for branch in branches {
                let mut branch_init = init.clone();
                if let WhenPattern::Pattern(p) = &branch.pattern {
                    declare_pattern_st(st, p, true, &mut branch_init);
                }
                if let Some(g) = &branch.guard {
                    expr(g, &mut branch_init, st);
                }
                match &branch.body {
                    ast::syntax::expr::WhenBody::Expr(e) => expr(e, &mut branch_init, st),
                    ast::syntax::expr::WhenBody::Block(b) => block(b, &mut branch_init, st),
                }
                merged = Some(match merged {
                    None => branch_init,
                    Some(prev) => initialized_after_branches(prev, branch_init),
                });
            }
            if let Some(merged) = merged {
                *init = merged;
            }
        }
        ExprKind::Error(_) | ExprKind::Missing(_) => {}
    }
}
