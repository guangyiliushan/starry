//! Pass 4：moveck —— Own 值的最小线性移动检查（M3.5）
//!
//! 近似规则（grill 已批）：逐 fun 体 / 顶层绑定初始化 walk；
//! Own 绑定的首次值使用即移动（`ExprKind::Ident` 且 def 为 LocalBinding/Param、
//! 类型为 `Named(Own, _)`）；赋值重置；分支/循环取两态并集
//! （过度拒绝可接受，漏报不可——见 semantic-notes）。
//! lambda/spawn/macro 内部 resolve 期即未解析，此处天然跳过。

use std::collections::BTreeSet;

use ast::syntax::decl::{DeclKind, FunctionBody};
use ast::syntax::expr::*;
use ast::syntax::node::SourceFile;
use ast::syntax::stmt::{StmtKind, StmtWrap};

use crate::SemState;
use crate::def::{DefId, DefKind};
use crate::ty::TyKind;

pub(crate) fn run(ast: &SourceFile, st: &mut SemState) {
    for item in &ast.items {
        match &item.kind {
            DeclKind::Fun(f) => {
                let mut moved = BTreeSet::new();
                match &f.body {
                    Some(FunctionBody::Block(b)) => block(b, st, &mut moved),
                    Some(FunctionBody::Expr(e)) => expr(e, st, &mut moved),
                    None => {}
                }
            }
            DeclKind::Binding(b) => {
                if let Some(init) = &b.init {
                    let mut moved = BTreeSet::new();
                    expr(init, st, &mut moved);
                }
            }
            _ => {}
        }
    }
}

fn is_own(def: DefId, st: &SemState) -> bool {
    let Some(own) = st.own_def else {
        return false;
    };
    matches!(
        st.tys.kind(st.defs.get(def).sig.ty),
        TyKind::Named(d, _) if *d == own
    )
}

/// 值使用判定：Own 绑定的 Ident 出现即移动（赋值目标由 stmt 层特判，不走此处）
fn use_ident(e: &ExprWrap, st: &mut SemState, moved: &mut BTreeSet<DefId>) {
    if let Some(def) = st.expr_def.get(e.id).copied()
        && matches!(
            st.defs.get(def).kind,
            DefKind::LocalBinding { .. } | DefKind::Param
        )
        && is_own(def, st)
        && !moved.insert(def)
    {
        st.diags.emit(ast::syntax::node::Diagnostic::error(
            ast::syntax::node::DiagCode::SemUseAfterMove,
            e.span,
            "use of a moved Own value",
        ));
    }
}

fn block(b: &Block, st: &mut SemState, moved: &mut BTreeSet<DefId>) {
    for s in &b.stmts {
        stmt(s, st, moved);
    }
    if let Some(t) = &b.tail {
        expr(t, st, moved);
    }
}

fn stmt(s: &StmtWrap, st: &mut SemState, moved: &mut BTreeSet<DefId>) {
    match &s.kind {
        StmtKind::Expr(es) => expr(&es.expr, st, moved),
        StmtKind::Assignment { target, op, value } => {
            expr(value, st, moved);
            if *op == ast::token::OperatorKind::Eq
                && let ExprKind::Ident(_) = &target.kind
                && let Some(def) = st.expr_def.get(target.id).copied()
                && matches!(
                    st.defs.get(def).kind,
                    DefKind::LocalBinding { .. } | DefKind::Param
                )
            {
                moved.remove(&def);
            }
        }
        StmtKind::Binding(b) => {
            if let Some(init) = &b.init {
                expr(init, st, moved);
            }
        }
        StmtKind::For { iterable, body, .. } => {
            expr(iterable, st, moved);
            let mut inner = moved.clone();
            block(body, st, &mut inner);
            moved.extend(inner);
        }
        StmtKind::While { cond, body, .. } => {
            condition(cond, st, moved);
            let mut inner = moved.clone();
            block(body, st, &mut inner);
            moved.extend(inner);
        }
        StmtKind::Loop { body, .. } => {
            let mut inner = moved.clone();
            block(body, st, &mut inner);
            moved.extend(inner);
        }
        StmtKind::Break { value, .. } => {
            if let Some(v) = value {
                expr(v, st, moved);
            }
        }
        StmtKind::Continue { .. } => {}
        StmtKind::Return { value } => {
            if let Some(v) = value {
                expr(v, st, moved);
            }
        }
        StmtKind::Defer { body } => block(body, st, moved),
        StmtKind::Comptime { block: b } => block(b, st, moved),
        StmtKind::Grow { expr: e, bits } => {
            expr(e, st, moved);
            if let Some(b) = bits {
                expr(b, st, moved);
            }
        }
        StmtKind::Error(_) | StmtKind::Missing(_) => {}
    }
}

fn condition(c: &Condition, st: &mut SemState, moved: &mut BTreeSet<DefId>) {
    match c {
        Condition::Expr(e) => expr(e, st, moved),
        Condition::Val { expr: e, .. } => expr(e, st, moved),
    }
}

fn expr(e: &ExprWrap, st: &mut SemState, moved: &mut BTreeSet<DefId>) {
    match &e.kind {
        ExprKind::Ident(_) => use_ident(e, st, moved),
        ExprKind::Binary { lhs, rhs, .. } => {
            expr(lhs, st, moved);
            expr(rhs, st, moved);
        }
        ExprKind::Is { lhs, .. } => expr(lhs, st, moved),
        ExprKind::In { lhs, rhs, .. } => {
            expr(lhs, st, moved);
            expr(rhs, st, moved);
        }
        ExprKind::Unary { operand, .. }
        | ExprKind::ForceUnwrap { expr: operand }
        | ExprKind::Await { expr: operand }
        | ExprKind::Cast { expr: operand, .. }
        | ExprKind::ArrayRepeat { value: operand, .. } => expr(operand, st, moved),
        ExprKind::Async { block: b } | ExprKind::Concurrent { block: b } => block(b, st, moved),
        ExprKind::Loop { body } => {
            let mut inner = moved.clone();
            block(body, st, &mut inner);
            moved.extend(inner);
        }
        ExprKind::Call { callee, args, .. } => {
            expr(callee, st, moved);
            for a in args {
                expr(&a.value, st, moved);
            }
        }
        ExprKind::Index { base, index, .. } => {
            expr(base, st, moved);
            expr(index, st, moved);
        }
        ExprKind::Member { base, .. } => expr(base, st, moved),
        ExprKind::StringInterp { parts } => {
            for p in parts {
                expr(p, st, moved);
            }
        }
        ExprKind::When { subject, branches } => {
            if let Some(s) = subject {
                expr(s, st, moved);
            }
            let base = moved.clone();
            let mut acc = base.clone();
            for b in branches {
                let mut state = base.clone();
                if let Some(g) = &b.guard {
                    expr(g, st, &mut state);
                }
                match &b.body {
                    WhenBody::Expr(e) => expr(e, st, &mut state),
                    WhenBody::Block(b) => block(b, st, &mut state),
                }
                acc.extend(state);
            }
            *moved = acc;
        }
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => {
            condition(cond, st, moved);
            let saved = moved.clone();
            block(then_block, st, moved);
            let then_moved = std::mem::replace(moved, saved);
            if let Some(eb) = else_block {
                block(eb, st, moved);
            }
            moved.extend(then_moved);
        }
        ExprKind::Tuple(xs) | ExprKind::Array(xs) => {
            for x in xs {
                expr(x, st, moved);
            }
        }
        ExprKind::Comptime { block: b } | ExprKind::Unsafe { block: b } => block(b, st, moved),
        // resolve 期未解析的区域：无 expr_def，天然跳过
        ExprKind::Lambda { .. } | ExprKind::Spawn(_) | ExprKind::MacroCall { .. } => {}
        _ => {}
    }
}
