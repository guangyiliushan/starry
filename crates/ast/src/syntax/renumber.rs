//! M7 单态化克隆的 NodeId 重编号（sem mono 专用契约）。
//!
//! 克隆声明与原件共享 span（诊断回指源位置），但 NodeId 必须全局唯一：
//! 侧表（expr_ty/expr_def/…）按 NodeId 稠密索引，撞 id = 语义串味。
//! 自 `next` 起顺序编号，调用方负责把侧表 grow 到返回后的 `next`。

use super::decl::*;
use super::expr::*;
use super::node::{NodeId, Path};
use super::pat::*;
use super::stmt::*;
use super::ty::*;

fn nid(next: &mut u32) -> NodeId {
    let id = NodeId(*next);
    *next += 1;
    id
}

// Path / PathSegment 不携带 NodeId，仅为对称保留遍历点
fn path(_: &mut Path, _: &mut u32) {}

pub fn renumber_decl(decl: &mut DeclNode, next: &mut u32) {
    decl.id = nid(next);
    renumber_decl_kind(&mut decl.kind, next);
}

fn renumber_decl_kind(kind: &mut DeclKind, next: &mut u32) {
    match kind {
        DeclKind::Fun(f) => {
            for tp in &mut f.type_params {
                tp.id = nid(next);
                if let TypeParamName::Error(e) = &mut tp.name {
                    e.id = nid(next);
                }
                for b in &mut tp.bounds {
                    renumber_ty(b, next);
                }
                if let Some(d) = &mut tp.default {
                    renumber_ty(d, next);
                }
            }
            for p in &mut f.params {
                if let Some(t) = &mut p.ty {
                    renumber_ty(t, next);
                }
                if let Some(d) = &mut p.default {
                    renumber_expr(d, next);
                }
            }
            if let Some(r) = &mut f.ret {
                renumber_ty(r, next);
            }
            for w in &mut f.where_clause {
                renumber_ty(&mut w.ty, next);
                for b in &mut w.bounds {
                    renumber_ty(b, next);
                }
            }
            if let Some(b) = &mut f.body {
                match b {
                    FunctionBody::Block(b) => renumber_block(b, next),
                    FunctionBody::Expr(e) => renumber_expr(e, next),
                }
            }
        }
        // M7 slice 1 只克隆 Fun 声明；其余臂防御性覆盖（未来克隆面扩大时不用回填）
        DeclKind::Binding(b) => {
            renumber_pat(&mut b.pattern, next);
            if let Some(t) = &mut b.ty {
                renumber_ty(t, next);
            }
            if let Some(i) = &mut b.init {
                renumber_expr(i, next);
            }
        }
        DeclKind::Struct(s) => {
            for p in &mut s.primary_ctor {
                if let Some(t) = &mut p.ty {
                    renumber_ty(t, next);
                }
                if let Some(d) = &mut p.default {
                    renumber_expr(d, next);
                }
            }
            for f in &mut s.fields {
                renumber_ty(&mut f.ty, next);
            }
        }
        DeclKind::Enum(e) => {
            for v in &mut e.variants {
                for a in &mut v.args {
                    if let Some(t) = &mut a.ty {
                        renumber_ty(t, next);
                    }
                    if let Some(d) = &mut a.default {
                        renumber_expr(d, next);
                    }
                }
            }
        }
        _ => {}
    }
}

fn renumber_block(b: &mut Block, next: &mut u32) {
    for s in &mut b.stmts {
        renumber_stmt(s, next);
    }
    if let Some(t) = &mut b.tail {
        renumber_expr(t, next);
    }
}

fn renumber_stmt(s: &mut StmtWrap, next: &mut u32) {
    s.id = nid(next);
    match &mut s.kind {
        StmtKind::Expr(es) => renumber_expr(&mut es.expr, next),
        StmtKind::Assignment { target, value, .. } => {
            renumber_expr(target, next);
            renumber_expr(value, next);
        }
        StmtKind::Binding(b) => {
            renumber_pat(&mut b.pattern, next);
            if let Some(t) = &mut b.ty {
                renumber_ty(t, next);
            }
            if let Some(i) = &mut b.init {
                renumber_expr(i, next);
            }
        }
        StmtKind::For {
            pattern,
            iterable,
            body,
            ..
        } => {
            renumber_expr(iterable, next);
            renumber_pat(pattern, next);
            renumber_block(body, next);
        }
        StmtKind::While { cond, body, .. } => {
            renumber_condition(cond, next);
            renumber_block(body, next);
        }
        StmtKind::Loop { body, .. } => renumber_block(body, next),
        StmtKind::Break { value, .. } => {
            if let Some(v) = value {
                renumber_expr(v, next);
            }
        }
        StmtKind::Continue { .. } => {}
        StmtKind::Return { value } => {
            if let Some(v) = value {
                renumber_expr(v, next);
            }
        }
        StmtKind::Defer { body } => renumber_block(body, next),
        StmtKind::Comptime { block } => renumber_block(block, next),
        StmtKind::Grow { expr, bits } => {
            renumber_expr(expr, next);
            if let Some(b) = bits {
                renumber_expr(b, next);
            }
        }
        StmtKind::Error(e) => e.id = nid(next),
        StmtKind::Missing(m) => m.id = nid(next),
    }
}

fn renumber_condition(c: &mut Condition, next: &mut u32) {
    match c {
        Condition::Expr(e) => renumber_expr(e, next),
        Condition::Val { pattern, expr } => {
            renumber_pat(pattern, next);
            renumber_expr(expr, next);
        }
    }
}

fn renumber_expr(e: &mut ExprWrap, next: &mut u32) {
    e.id = nid(next);
    match &mut e.kind {
        ExprKind::Binary { lhs, rhs, .. } => {
            renumber_expr(lhs, next);
            renumber_expr(rhs, next);
        }
        ExprKind::Is { lhs, pattern, .. } => {
            renumber_expr(lhs, next);
            renumber_pat(pattern, next);
        }
        ExprKind::In { lhs, rhs, .. } => {
            renumber_expr(lhs, next);
            renumber_expr(rhs, next);
        }
        ExprKind::Unary { operand, .. } => renumber_expr(operand, next),
        ExprKind::AddrOf { expr, .. } => renumber_expr(expr, next),
        ExprKind::Deref { expr } => renumber_expr(expr, next),
        ExprKind::Cast { expr, ty, .. } => {
            renumber_expr(expr, next);
            renumber_ty(ty, next);
        }
        ExprKind::Call {
            callee,
            type_args,
            args,
        } => {
            renumber_expr(callee, next);
            for ta in type_args {
                ta.id = nid(next);
                renumber_ty(&mut ta.ty, next);
            }
            for a in args {
                renumber_expr(&mut a.value, next);
            }
        }
        ExprKind::Index { base, index, .. } => {
            renumber_expr(base, next);
            renumber_expr(index, next);
        }
        ExprKind::Member { base, .. } => renumber_expr(base, next),
        ExprKind::ForceUnwrap { expr } => renumber_expr(expr, next),
        ExprKind::Lambda { params, body } => {
            match params {
                LambdaParams::It => {}
                LambdaParams::Explicit(ps) => {
                    for p in ps {
                        renumber_pat(p, next);
                    }
                }
            }
            renumber_block(body, next);
        }
        ExprKind::Loop { body, .. } => renumber_block(body, next),
        ExprKind::StringInterp { parts } => {
            for p in parts {
                renumber_expr(p, next);
            }
        }
        ExprKind::When { subject, branches } => {
            if let Some(s) = subject {
                renumber_expr(s, next);
            }
            for b in branches {
                match &mut b.pattern {
                    WhenPattern::Expression(e) => renumber_expr(e, next),
                    WhenPattern::Pattern(p) => renumber_pat(p, next),
                    WhenPattern::Else => {}
                }
                if let Some(g) = &mut b.guard {
                    renumber_expr(g, next);
                }
                match &mut b.body {
                    WhenBody::Expr(e) => renumber_expr(e, next),
                    WhenBody::Block(b) => renumber_block(b, next),
                }
            }
        }
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => {
            renumber_condition(cond, next);
            renumber_block(then_block, next);
            if let Some(eb) = else_block {
                renumber_block(eb, next);
            }
        }
        ExprKind::Tuple(xs) | ExprKind::Array(xs) => {
            for x in xs {
                renumber_expr(x, next);
            }
        }
        ExprKind::ArrayRepeat { value, .. } => renumber_expr(value, next),
        ExprKind::Spawn(s) => {
            if let Some(a) = &mut s.actor {
                path(&mut a.name, next);
                for arg in &mut a.args {
                    renumber_expr(&mut arg.value, next);
                }
            }
            renumber_block(&mut s.body, next);
        }
        ExprKind::Async { block }
        | ExprKind::Comptime { block }
        | ExprKind::Unsafe { block }
        | ExprKind::Concurrent { block } => renumber_block(block, next),
        ExprKind::Await { expr } => renumber_expr(expr, next),
        ExprKind::Super { suffix } => {
            if let SuperSuffix::Call(args) = suffix {
                for a in args {
                    renumber_expr(&mut a.value, next);
                }
            }
        }
        // MacroCall 的 TokenTree 不携带 NodeId；叶子无子节点
        ExprKind::Literal(_)
        | ExprKind::Bool(_)
        | ExprKind::Null
        | ExprKind::Ident(_)
        | ExprKind::Path(_)
        | ExprKind::MacroCall { .. }
        | ExprKind::Proceed => {}
        ExprKind::Error(e) => e.id = nid(next),
        ExprKind::Missing(m) => m.id = nid(next),
    }
}

fn renumber_pat(p: &mut PatWrap, next: &mut u32) {
    p.id = nid(next);
    match &mut p.kind {
        PatKind::Or(ps) => {
            for p in ps {
                renumber_pat(p, next);
            }
        }
        PatKind::Binding { ty, .. } => {
            if let Some(t) = ty {
                renumber_ty(t, next);
            }
        }
        PatKind::Tuple { elements, .. } | PatKind::Array { elements, .. } => {
            for p in elements {
                renumber_pat(p, next);
            }
        }
        PatKind::Constructor { path: cp, args } => {
            path(cp, next);
            for a in args {
                renumber_pat(a, next);
            }
        }
        PatKind::Struct { path: cp, fields } => {
            path(cp, next);
            for (_, fp) in fields {
                renumber_pat(fp, next);
            }
        }
        PatKind::Range { lo, hi, .. } => {
            renumber_pat(lo, next);
            renumber_pat(hi, next);
        }
        PatKind::Literal(_) | PatKind::Bool(_) | PatKind::Null | PatKind::Wildcard => {}
        PatKind::Error(e) => e.id = nid(next),
        PatKind::Missing(m) => m.id = nid(next),
    }
}

fn renumber_ty(t: &mut TypeWrap, next: &mut u32) {
    t.id = nid(next);
    match &mut t.kind {
        TypeKind::Ref { inner, .. } | TypeKind::Pointer { inner, .. } => {
            renumber_ty(inner, next);
        }
        TypeKind::Union(ts) => {
            for t in ts {
                renumber_ty(t, next);
            }
        }
        TypeKind::Named {
            path: cp,
            type_args,
        } => {
            path(cp, next);
            for a in type_args {
                a.id = nid(next);
                renumber_ty(&mut a.ty, next);
            }
        }
        TypeKind::Dyn(inner) => renumber_ty(inner, next),
        TypeKind::SelfType => {}
        TypeKind::Nullable(inner) => renumber_ty(inner, next),
        TypeKind::Fn { params, ret } => {
            for p in params {
                renumber_ty(p, next);
            }
            renumber_ty(ret, next);
        }
        TypeKind::Array { element, .. } => renumber_ty(element, next),
        TypeKind::Tuple(ts) => {
            for t in ts {
                renumber_ty(t, next);
            }
        }
        TypeKind::Unit => {}
        TypeKind::Error(e) => e.id = nid(next),
        TypeKind::Missing(m) => m.id = nid(next),
    }
}
