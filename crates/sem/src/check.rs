//! Pass 3：check —— 自底向上定型，写 expr_ty；`Error` 类型投毒抑制级联
//!
//! 教科书依据：龙书 6.5（类型检查）+ Tiger ch.5（环境 + 定型规则）。
//! 提升格规则见 semantic-notes（整型加宽 / int→float / Null→Nullable）。

use ast::interner::Symbol;
use ast::syntax::decl::{BindingDecl, DeclKind, FunctionBody};
use ast::syntax::expr::*;
use ast::syntax::node::{DiagCode, Diagnostic, NodeId, SourceFile};
use ast::syntax::pat::{PatKind, PatWrap};
use ast::syntax::stmt::{StmtKind, StmtWrap};
use ast::syntax::ty::TypeArg;
use ast::token::{LiteralKind, OperatorKind, Span};

use crate::declare::member_lookup;
use crate::def::{DefId, DefKind};
use crate::ty::{TyId, TyKind};
use crate::{LoopFrame, PatRes, SemState};

pub(crate) fn run(ast: &SourceFile, st: &mut SemState) {
    let root = st.scopes.root();
    for item in &ast.items {
        match &item.kind {
            DeclKind::Fun(f) => check_fun(item.id, f, st),
            DeclKind::Binding(b) => check_top_binding(b, st, root),
            _ => {}
        }
    }
    // 依赖序单遍推断的收尾：仍为 Unknown 的顶层绑定 = 无法推定，
    // 钉一条诊断（Unknown 不投毒，使用点也会各自诊断）
    for item in &ast.items {
        if let DeclKind::Binding(b) = &item.kind
            && b.init.is_some()
            && let PatKind::Binding { name, .. } = &b.pattern.kind
            && let Some(def) = st.scopes.lookup_local(root, *name)
            && st.tys.is_unknown(st.defs.get(def).sig.ty)
        {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemTypeMismatch,
                b.pattern.span,
                format!(
                    "cannot infer the type of `{}` (dependency not resolvable in source order)",
                    st.interner.resolve(*name)
                ),
            ));
        }
    }
    #[cfg(debug_assertions)]
    verify::run(ast, st);
}

fn check_fun(decl_id: NodeId, f: &ast::syntax::decl::FunDecl, st: &mut SemState) {
    let root = st.scopes.root();
    let Some(fdef) = st.scopes.lookup_local(root, f.name) else {
        return;
    };
    if !matches!(st.defs.get(fdef).kind, DefKind::Fun) {
        return;
    }
    let ret = st.defs.get(fdef).sig.ret;
    st.funs.push(crate::FunFrame {
        node: decl_id,
        def: fdef,
    });
    let tail_ty = match &f.body {
        Some(FunctionBody::Block(b)) => {
            let ty = type_block(b, st, true);
            b.tail.as_ref().map(|_| ty)
        }
        Some(FunctionBody::Expr(e)) => Some(type_expr(e, st, true)),
        None => None,
    };
    // 返回经由 return 语句逐点核查；块体无尾表达式时不比较。
    // 仅声明了返回型才与尾表达式比较：未声明返回型按 unit 推断，
    // 尾表达式求值但不核查——`fun main() { helper() }` 是惯用形状，
    // 不能假阳性；`: unit` 与未声明的差异记入 semantic-notes
    if f.ret.is_some()
        && let Some(body_ty) = tail_ty
        && !st.tys.assignable(body_ty, ret)
    {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemTypeMismatch,
            f.ret
                .as_ref()
                .map(|t| t.span)
                .unwrap_or(Span { start: 0, end: 0 }),
            "function body type does not match the declared return type",
        ));
    }
    st.funs.pop();
}

fn check_top_binding(b: &BindingDecl, st: &mut SemState, root: crate::def::ScopeId) {
    let Some(init) = &b.init else {
        return;
    };
    let init_ty = type_expr(init, st, true);
    if let PatKind::Binding { name, .. } = &b.pattern.kind
        && let Some(def) = st.scopes.lookup_local(root, *name)
    {
        let ann = st.defs.get(def).sig.ty;
        if st.tys.needs_infer(ann) {
            st.defs.get_mut(def).sig.ty = init_ty;
        } else if !st.tys.assignable(init_ty, ann) {
            mismatch(st, init.span);
        }
    }
}

fn type_block(b: &Block, st: &mut SemState, in_value: bool) -> TyId {
    for s in &b.stmts {
        type_stmt(s, st);
    }
    match &b.tail {
        Some(t) => type_expr(t, st, in_value),
        None => st.tys.unit,
    }
}

fn type_stmt(s: &StmtWrap, st: &mut SemState) {
    match &s.kind {
        StmtKind::Expr(es) => {
            type_expr(&es.expr, st, false);
        }
        StmtKind::Assignment { target, op, value } => {
            let target_ty = assign_target_ty(target, type_expr(target, st, false), st);
            let value_ty = type_expr(value, st, true);
            let op = *op;
            match op {
                OperatorKind::Eq => {
                    if !st.tys.assignable(value_ty, target_ty) {
                        mismatch(st, value.span);
                    }
                }
                OperatorKind::PlusEq
                | OperatorKind::MinusEq
                | OperatorKind::StarEq
                | OperatorKind::SlashEq
                | OperatorKind::PercentEq
                | OperatorKind::BitAndEq
                | OperatorKind::BitOrEq
                | OperatorKind::BitXorEq
                | OperatorKind::ShlEq
                | OperatorKind::ShrEq => {
                    let ok = st.tys.assignable(value_ty, target_ty)
                        || st.tys.join(value_ty, target_ty) == Some(target_ty);
                    if !ok {
                        mismatch(st, value.span);
                    }
                }
                _ => {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemUnsupportedConstruct,
                        s.span,
                        "unsupported assignment operator",
                    ));
                }
            }
        }
        StmtKind::Binding(b) => check_local_binding(b, st),
        StmtKind::For {
            pattern,
            iterable,
            body,
            ..
        } => {
            let iter_ty = type_expr(iterable, st, true);
            let elem = match st.tys.kind(iter_ty).clone() {
                TyKind::Array(t, _) => Some(t),
                TyKind::Range(t) => Some(t),
                TyKind::Error => None,
                _ => {
                    mismatch(st, iterable.span);
                    None
                }
            };
            if let Some(t) = elem {
                refine_bindings(pattern, t, st);
            }
            type_block(body, st, false);
        }
        StmtKind::While { cond, body, .. } => {
            check_condition(cond, st);
            type_block(body, st, false);
        }
        StmtKind::Loop { body, .. } => {
            st.loops.push(LoopFrame {
                break_values: Vec::new(),
            });
            type_block(body, st, false);
            let _ = st.loops.pop();
        }
        StmtKind::Break { value, .. } => {
            if let Some(v) = value {
                let vty = type_expr(v, st, true);
                if let Some(frame) = st.loops.last_mut() {
                    frame.break_values.push(vty);
                }
            }
        }
        StmtKind::Continue { .. } => {}
        StmtKind::Return { value } => {
            let Some(fun) = st.funs.last().copied() else {
                if let Some(v) = value {
                    type_expr(v, st, true);
                }
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemReturnOutsideFun,
                    s.span,
                    "return outside any function",
                ));
                return;
            };
            let ret = st.defs.get(fun.def).sig.ret;
            if let Some(v) = value {
                let vty = type_expr(v, st, true);
                if !st.tys.assignable(vty, ret) {
                    mismatch(st, v.span);
                }
            } else if !st.tys.assignable(st.tys.unit, ret) {
                // 裸 return 等价返回 unit：非 unit 返回型报错
                mismatch(st, s.span);
            }
        }
        StmtKind::Defer { body } => {
            type_block(body, st, false);
        }
        StmtKind::Comptime { block } => {
            type_block(block, st, false);
        }
        StmtKind::Grow { expr, bits } => {
            type_expr(expr, st, true);
            if let Some(b) = bits {
                let bty = type_expr(b, st, true);
                if !st.tys.is_integral(bty) && !st.tys.is_error(bty) {
                    mismatch(st, b.span);
                }
            }
        }
        StmtKind::Error(_) | StmtKind::Missing(_) => {}
    }
}

fn check_local_binding(b: &BindingDecl, st: &mut SemState) {
    let Some(init) = &b.init else {
        if b.ty.is_none() {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                b.pattern.span,
                "binding requires an initializer",
            ));
        }
        return;
    };
    let init_ty = type_expr(init, st, true);
    match &b.pattern.kind {
        PatKind::Binding { .. } => {
            if let Some(&def) = st.pat_binding_def.get(b.pattern.id) {
                let ann = st.defs.get(def).sig.ty;
                if st.tys.needs_infer(ann) {
                    st.defs.get_mut(def).sig.ty = init_ty;
                } else if !st.tys.assignable(init_ty, ann) {
                    mismatch(st, init.span);
                }
            }
        }
        PatKind::Wildcard => {}
        PatKind::Tuple { elements, .. } | PatKind::Array { elements, .. } => {
            let elem_tys = match st.tys.kind(init_ty).clone() {
                TyKind::Tuple(ts) => Some(ts),
                TyKind::Array(t, _) => Some(vec![t; elements.len()]),
                TyKind::Error => None,
                _ => {
                    mismatch(st, init.span);
                    None
                }
            };
            if let Some(ts) = elem_tys {
                if ts.len() != elements.len() {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemArityMismatch,
                        init.span,
                        "destructuring arity mismatch",
                    ));
                }
                for (p, t) in elements.iter().zip(ts.iter()) {
                    refine_bindings(p, *t, st);
                }
            }
        }
        _ => {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                b.pattern.span,
                "only binding/wildcard/tuple/array patterns are supported in bindings",
            ));
        }
    }
}

fn refine_bindings(pat: &PatWrap, ty: TyId, st: &mut SemState) {
    match &pat.kind {
        PatKind::Binding { .. } => {
            if let Some(&def) = st.pat_binding_def.get(pat.id) {
                let cur = st.defs.get(def).sig.ty;
                if st.tys.needs_infer(cur) {
                    st.defs.get_mut(def).sig.ty = ty;
                }
            }
        }
        PatKind::Or(ps)
        | PatKind::Tuple { elements: ps, .. }
        | PatKind::Array { elements: ps, .. } => {
            for p in ps {
                refine_bindings(p, ty, st);
            }
        }
        _ => {}
    }
}
/// 赋值目标分类：target 已由调用方 `type_expr` 定型（每节点恰好一次），
/// 此处只做可赋值性诊断并返回目标类型
fn assign_target_ty(target: &ExprWrap, target_ty: TyId, st: &mut SemState) -> TyId {
    match &target.kind {
        ExprKind::Ident(_) => match st.expr_def.get(target.id).copied() {
            Some(def) => match &st.defs.get(def).kind {
                DefKind::LocalBinding { mutable } => {
                    if !mutable {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemAssignToVal,
                            target.span,
                            "cannot assign to an immutable binding",
                        ));
                    }
                    st.defs.get(def).sig.ty
                }
                DefKind::Param => {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemAssignToVal,
                        target.span,
                        "parameters are immutable in semantic v1",
                    ));
                    st.tys.error
                }
                _ => {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemAssignToVal,
                        target.span,
                        "not an assignable binding",
                    ));
                    st.tys.error
                }
            },
            None => st.tys.error,
        },
        ExprKind::Member { .. } | ExprKind::Index { .. } => target_ty,
        _ => {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemAssignToVal,
                target.span,
                "not an assignable target",
            ));
            st.tys.error
        }
    }
}
fn member_field_ty(base_ty: TyId, name: Symbol, safe: bool, span: Span, st: &mut SemState) -> TyId {
    let mut ty = base_ty;
    let mut nullable_base = false;
    if let TyKind::Nullable(inner) = st.tys.kind(ty).clone()
        && safe
    {
        ty = inner;
        nullable_base = true;
    }
    let result = match st.tys.kind(ty).clone() {
        TyKind::Named(def, _) => match &st.defs.get(def).kind {
            // 枚举成员访问 = 变体值（构造器亦可，调用点另行核查实参）：
            // `E.A` / `Opt.Some` 的类型即所属枚举
            DefKind::Enum => match member_lookup(st, def, name) {
                Some(vdef) if matches!(st.defs.get(vdef).kind, DefKind::EnumVariant { .. }) => {
                    st.tys.builtin_named(def)
                }
                _ => {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemUnknownMember,
                        span,
                        format!("unknown variant `{}`", st.interner.resolve(name)),
                    ));
                    st.tys.error
                }
            },
            DefKind::Struct => {
                let fields = st.defs.get(def).sig.fields.clone();
                match fields.iter().find(|(n, _)| *n == name) {
                    Some((_, t)) => *t,
                    None => {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnknownMember,
                            span,
                            format!("unknown field `{}`", st.interner.resolve(name)),
                        ));
                        st.tys.error
                    }
                }
            }
            _ => {
                unsupported(
                    st,
                    span,
                    "member access is supported on struct values only in semantic v1",
                );
                st.tys.error
            }
        },
        TyKind::Error => st.tys.error,
        _ => {
            unsupported(
                st,
                span,
                "member access is supported on struct values only in semantic v1",
            );
            st.tys.error
        }
    };
    if nullable_base && !st.tys.is_error(result) {
        st.tys.nullable(result)
    } else {
        result
    }
}

fn check_condition(cond: &Condition, st: &mut SemState) {
    match cond {
        Condition::Expr(e) => {
            let ty = type_expr(e, st, true);
            require_bool(ty, e.span, st);
        }
        Condition::Val { pattern, expr } => {
            let ty = type_expr(expr, st, true);
            refine_bindings(pattern, ty, st);
        }
    }
}

fn require_bool(ty: TyId, span: Span, st: &mut SemState) {
    if ty != st.tys.bool_ && !st.tys.is_error(ty) {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemConditionNotBool,
            span,
            "condition must be Boolean",
        ));
    }
}

fn unsupported(st: &mut SemState, span: Span, message: &'static str) {
    st.diags.emit(Diagnostic::error(
        DiagCode::SemUnsupportedConstruct,
        span,
        message,
    ));
}

fn mismatch(st: &mut SemState, span: Span) {
    st.diags.emit(Diagnostic::error(
        DiagCode::SemTypeMismatch,
        span,
        "type mismatch",
    ));
}

/// 表达式定型入口：总是写 expr_ty（含 Error），树遍历每节点恰好一次
pub(crate) fn type_expr(e: &ExprWrap, st: &mut SemState, in_value: bool) -> TyId {
    #[cfg(debug_assertions)]
    debug_assert!(
        st.expr_ty.get(e.id).is_none(),
        "expr typed twice: coverage drift at node {:?}",
        e.id
    );
    let ty = type_expr_inner(e, st, in_value);
    st.expr_ty.set(e.id, ty);
    ty
}

fn type_expr_inner(e: &ExprWrap, st: &mut SemState, in_value: bool) -> TyId {
    match &e.kind {
        ExprKind::Error(_) | ExprKind::Missing(_) => st.tys.error,
        ExprKind::Literal(lk) => match lk {
            LiteralKind::Integer => st.tys.default_int,
            LiteralKind::Float => st.tys.default_float,
            LiteralKind::String => st.tys.string,
            LiteralKind::Char => st.tys.char_,
        },
        ExprKind::Bool(_) => st.tys.bool_,
        ExprKind::Null => st.tys.null,
        ExprKind::Ident(_) | ExprKind::Path(_) => match st.expr_def.get(e.id).copied() {
            Some(def) => def_value_ty(def, st),
            None => st.tys.error,
        },
        ExprKind::Unary { op, operand } => {
            let o = type_expr(operand, st, true);
            match op {
                OperatorKind::Bang => {
                    require_bool(o, operand.span, st);
                    st.tys.bool_
                }
                OperatorKind::Minus => {
                    if !st.tys.is_numeric(o) && !st.tys.is_error(o) {
                        mismatch(st, operand.span);
                    }
                    o
                }
                OperatorKind::BitNot => {
                    if !st.tys.is_integral(o) && !st.tys.is_error(o) {
                        mismatch(st, operand.span);
                    }
                    o
                }
                _ => {
                    unsupported(
                        st,
                        e.span,
                        "this unary operator is not supported in semantic v1",
                    );
                    st.tys.error
                }
            }
        }
        ExprKind::Binary { op, lhs, rhs } => type_binary(*op, lhs, rhs, e.span, st),
        ExprKind::Is { lhs, pattern, .. } => {
            let lty = type_expr(lhs, st, true);
            refine_pattern_types(pattern, lty, st);
            st.tys.bool_
        }
        ExprKind::In { lhs, pattern, .. } => {
            let lty = type_expr(lhs, st, true);
            let subject = match st.tys.kind(lty).clone() {
                TyKind::Array(t, _) | TyKind::Range(t) => Some(t),
                TyKind::Error => None,
                _ => {
                    mismatch(st, lhs.span);
                    None
                }
            };
            if let Some(s) = subject {
                refine_pattern_types(pattern, s, st);
            }
            st.tys.bool_
        }
        ExprKind::Cast { expr, ty, kind } => {
            type_expr(expr, st, true);
            let target = st.ty_of_node.get(ty.id).copied().unwrap_or(st.tys.error);
            match kind {
                CastKind::As | CastKind::AsBang => target,
                CastKind::AsQuestion => {
                    if st.tys.is_error(target) {
                        st.tys.error
                    } else {
                        st.tys.nullable(target)
                    }
                }
            }
        }
        ExprKind::Call {
            callee,
            type_args,
            args,
        } => type_call(callee, type_args, args, e.span, st),
        ExprKind::Index { base, index } => {
            let bty = type_expr(base, st, true);
            let ity = type_expr(index, st, true);
            if !st.tys.is_integral(ity) && !st.tys.is_error(ity) {
                mismatch(st, index.span);
            }
            match st.tys.kind(bty).clone() {
                TyKind::Array(t, _) => t,
                TyKind::Error => st.tys.error,
                _ => {
                    mismatch(st, base.span);
                    st.tys.error
                }
            }
        }
        ExprKind::Member { base, name, safe } => {
            let bty = type_expr(base, st, true);
            member_field_ty(bty, *name, *safe, e.span, st)
        }
        ExprKind::ForceUnwrap { expr } => {
            let o = type_expr(expr, st, true);
            match st.tys.kind(o).clone() {
                TyKind::Nullable(t) => t,
                TyKind::Error => st.tys.error,
                _ => {
                    mismatch(st, expr.span);
                    st.tys.error
                }
            }
        }
        ExprKind::Tuple(xs) => {
            let mut ts = Vec::with_capacity(xs.len());
            for x in xs {
                ts.push(type_expr(x, st, true));
            }
            st.tys.intern(TyKind::Tuple(ts))
        }
        ExprKind::Array(xs) => {
            if xs.is_empty() {
                unsupported(
                    st,
                    e.span,
                    "cannot infer the element type of an empty array literal",
                );
                return st.tys.error;
            }
            let mut acc = type_expr(&xs[0], st, true);
            for x in &xs[1..] {
                let t = type_expr(x, st, true);
                match st.tys.join(acc, t) {
                    Some(j) => acc = j,
                    None => {
                        mismatch(st, x.span);
                        acc = st.tys.error;
                        // 不 break：剩余元素仍须定型（coverage 纪律，防 verify 断言）
                    }
                }
            }
            let n = xs.len() as u32;
            st.tys.intern(TyKind::Array(acc, n))
        }
        ExprKind::ArrayRepeat { value, count } => {
            let v = type_expr(value, st, true);
            st.tys.intern(TyKind::Array(v, *count))
        }
        ExprKind::StringInterp { parts } => {
            for p in parts {
                type_expr(p, st, true);
            }
            st.tys.string
        }
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => {
            check_condition(cond, st);
            let then_ty = type_block(then_block, st, in_value);
            match else_block {
                Some(eb) => {
                    let else_ty = type_block(eb, st, in_value);
                    st.tys.join(then_ty, else_ty).unwrap_or_else(|| {
                        mismatch(st, e.span);
                        st.tys.error
                    })
                }
                None => then_ty,
            }
        }
        ExprKind::When { subject, branches } => {
            type_when(subject.as_deref(), branches, e.span, in_value, st)
        }
        ExprKind::Await { expr } => type_expr(expr, st, in_value),
        ExprKind::Async { expr } => type_expr(expr, st, in_value),
        ExprKind::Concurrent { block } => type_block(block, st, in_value),
        ExprKind::Comptime { block } | ExprKind::Unsafe { block } => {
            type_block(block, st, in_value)
        }
        ExprKind::Lambda { .. } => {
            unsupported(st, e.span, "lambda typing is not supported in semantic v1");
            st.tys.error
        }
        ExprKind::Spawn(_) | ExprKind::Proceed | ExprKind::Super { .. } => {
            unsupported(st, e.span, "this construct is not supported in semantic v1");
            st.tys.error
        }
        ExprKind::MacroCall { .. } => {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemMacroExpansion,
                e.span,
                "macro expansion is a pre-pass and is not implemented yet",
            ));
            st.tys.error
        }
    }
}
// ==================== 二元运算 / 调用 / when ====================

/// 二元运算定型：先各定型一侧（Error 投毒直接短路），再按算子类别汇合
fn type_binary(
    op: OperatorKind,
    lhs: &ExprWrap,
    rhs: &ExprWrap,
    span: Span,
    st: &mut SemState,
) -> TyId {
    use OperatorKind as Op;
    let l = type_expr(lhs, st, true);
    let r = type_expr(rhs, st, true);
    if st.tys.is_error(l) || st.tys.is_error(r) {
        return st.tys.error;
    }
    match op {
        Op::Plus | Op::Minus | Op::Star | Op::Slash | Op::Percent => match st.tys.join(l, r) {
            Some(t) if st.tys.is_numeric(t) => t,
            _ => {
                mismatch(st, span);
                st.tys.error
            }
        },
        Op::BitAnd | Op::BitOr | Op::BitXor => {
            if st.tys.is_integral(l) && st.tys.is_integral(r) {
                st.tys.join(l, r).unwrap_or(l)
            } else {
                mismatch(st, span);
                st.tys.error
            }
        }
        Op::Shl | Op::Shr => {
            if st.tys.is_integral(l) && st.tys.is_integral(r) {
                l
            } else {
                mismatch(st, span);
                st.tys.error
            }
        }
        Op::EqEq | Op::BangEq => {
            // 等值按可汇合判定（命名/数组/元组结构相等即可比）
            if st.tys.join(l, r).is_some() {
                st.tys.bool_
            } else {
                mismatch(st, span);
                st.tys.error
            }
        }
        Op::Lt | Op::LtEq | Op::Gt | Op::GtEq => {
            // 序比较限定可序类型（数值/字符）：string < string、unit < unit 拒绝
            if st.tys.is_orderable(l) && st.tys.is_orderable(r) {
                st.tys.bool_
            } else {
                mismatch(st, span);
                st.tys.error
            }
        }
        Op::And | Op::Or => {
            require_bool(l, lhs.span, st);
            require_bool(r, rhs.span, st);
            st.tys.bool_
        }
        Op::DotDot | Op::DotDotEq => match st.tys.join(l, r) {
            Some(t) => st.tys.intern(TyKind::Range(t)),
            None => {
                mismatch(st, span);
                st.tys.error
            }
        },
        Op::QuestionQuestion => {
            let inner = match st.tys.kind(l).clone() {
                TyKind::Nullable(t) => t,
                _ => {
                    mismatch(st, lhs.span);
                    return st.tys.error;
                }
            };
            if st.tys.assignable(r, inner) {
                inner
            } else {
                mismatch(st, rhs.span);
                st.tys.error
            }
        }
        _ => {
            unsupported(
                st,
                span,
                "this binary operator is not supported in semantic v1",
            );
            st.tys.error
        }
    }
}

/// 调用定型：callee 先定型（coverage 需要）；位置实参按序入槽，
/// 命名实参按名入槽，默认值补位，最后逐槽 assignable 核查
fn type_call(
    callee: &ExprWrap,
    type_args: &[TypeArg],
    args: &[CallArg],
    span: Span,
    st: &mut SemState,
) -> TyId {
    let callee_ty = type_expr(callee, st, true);
    if !type_args.is_empty() {
        unsupported(
            st,
            span,
            "generic call instantiation is not supported in semantic v1",
        );
    }
    // 构造器调用：callee_ty 为 Named(struct)（P(...)）或 Named(enum)
    // （Opt.Some(...) / E.A(...)——成员访问枚举在 member_field_ty 已给枚举型）
    if let TyKind::Named(named, _) = st.tys.kind(callee_ty).clone() {
        match st.defs.get(named).kind.clone() {
            DefKind::Struct => {
                let fields = st.defs.get(named).sig.fields.clone();
                check_call_args(args, &fields, &[], span, st);
                return st.tys.builtin_named(named);
            }
            DefKind::Enum => {
                if let ExprKind::Member { name, .. } = &callee.kind {
                    let resolved =
                        member_lookup(st, named, *name).and_then(|v| match st.defs.get(v).kind {
                            DefKind::EnumVariant { parent, index } => Some((parent, index)),
                            _ => None,
                        });
                    if let Some((parent, index)) = resolved {
                        let tys = variant_arg_tys(parent, index, st);
                        check_variant_args(args, &tys, span, st);
                    } else {
                        type_all_args(args, st);
                    }
                    return st.tys.builtin_named(named);
                }
                type_all_args(args, st);
                return st.tys.error;
            }
            _ => {}
        }
    }
    // 具名函数：命名实参与默认值补位只走这条路径
    if let Some(def) = st.expr_def.get(callee.id).copied()
        && matches!(st.defs.get(def).kind, DefKind::Fun)
    {
        let params = st.defs.get(def).sig.params.clone();
        let defaults = st.defs.get(def).sig.param_defaults.clone();
        let ret = st.defs.get(def).sig.ret;
        check_call_args(args, &params, &defaults, span, st);
        return ret;
    }
    // 函数值调用（`(int) -> int` 型的形参/绑定作 callee）
    if let TyKind::Fn(ps, ret) = st.tys.kind(callee_ty).clone() {
        if args.len() != ps.len() {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemArityMismatch,
                span,
                "function value arity mismatch",
            ));
        }
        if args.iter().any(|a| a.name.is_some()) {
            unsupported(
                st,
                span,
                "named arguments are not supported for function values in semantic v1",
            );
        }
        for (a, p) in args.iter().zip(ps.iter()) {
            let at = type_expr(&a.value, st, true);
            if !st.tys.assignable(at, *p) {
                mismatch(st, a.value.span);
            }
        }
        for a in args.iter().skip(ps.len()) {
            type_expr(&a.value, st, true);
        }
        return ret;
    }
    // 有定义但不可调用（val/struct 名等）；函数值情形已在上方拦截
    if st.expr_def.get(callee.id).is_some() {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemNotCallable,
            callee.span,
            "callee is not a function",
        ));
        type_all_args(args, st);
        return st.tys.error;
    }
    // 成员调用 = 方法调用（v1 不支持）；构造器情形已在上方拦截
    if matches!(callee.kind, ExprKind::Member { .. }) {
        unsupported(
            st,
            callee.span,
            "method calls are not supported in semantic v1",
        );
        type_all_args(args, st);
        return st.tys.error;
    }
    st.diags.emit(Diagnostic::error(
        DiagCode::SemNotCallable,
        callee.span,
        "callee is not a function",
    ));
    type_all_args(args, st);
    st.tys.error
}

fn type_all_args(args: &[CallArg], st: &mut SemState) {
    for a in args {
        type_expr(&a.value, st, true);
    }
}

/// 实参定型共用槽位机：位置实参按序入槽、命名实参按名入槽、默认值补位、
/// 逐槽 assignable 核查；每个实参值恰好定型一次（coverage 纪律）
fn check_call_args(
    args: &[CallArg],
    params: &[(Symbol, TyId)],
    defaults: &[bool],
    span: Span,
    st: &mut SemState,
) {
    let mut slots: Vec<Option<TyId>> = vec![None; params.len()];
    let mut positional = 0usize;
    for arg in args {
        let idx = match arg.name {
            Some(name) => match params.iter().position(|(n, _)| *n == name) {
                Some(i) => i,
                None => {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemArityMismatch,
                        arg.value.span,
                        format!("unknown parameter name `{}`", st.interner.resolve(name)),
                    ));
                    type_expr(&arg.value, st, true);
                    continue;
                }
            },
            None => {
                let i = positional;
                positional += 1;
                if i >= params.len() {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemArityMismatch,
                        arg.value.span,
                        "too many positional arguments",
                    ));
                    type_expr(&arg.value, st, true);
                    continue;
                }
                i
            }
        };
        if slots[idx].is_some() {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemArityMismatch,
                arg.value.span,
                "duplicate argument",
            ));
            type_expr(&arg.value, st, true);
            continue;
        }
        let aty = type_expr(&arg.value, st, true);
        slots[idx] = Some(aty);
    }
    for i in 0..params.len() {
        if slots[i].is_none() {
            if i < defaults.len() && defaults[i] {
                slots[i] = Some(params[i].1);
            } else {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemArityMismatch,
                    span,
                    format!(
                        "missing argument for parameter `{}`",
                        st.interner.resolve(params[i].0)
                    ),
                ));
                slots[i] = Some(st.tys.error);
            }
        }
    }
    for (i, slot) in slots.iter().enumerate() {
        if !st.tys.assignable(slot.unwrap_or(st.tys.error), params[i].1) {
            mismatch(st, span);
        }
    }
}

/// 变体构造实参：仅位置形；逐个定型 + arity / assignable 核查
fn check_variant_args(args: &[CallArg], tys: &[TyId], span: Span, st: &mut SemState) {
    if args.len() != tys.len() {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemArityMismatch,
            span,
            "variant argument count mismatch",
        ));
    }
    if args.iter().any(|a| a.name.is_some()) {
        unsupported(
            st,
            span,
            "named arguments are not supported for enum variant constructors in semantic v1",
        );
    }
    for (a, t) in args.iter().zip(tys.iter()) {
        let at = type_expr(&a.value, st, true);
        if !st.tys.assignable(at, *t) {
            mismatch(st, a.value.span);
        }
    }
    for a in args.iter().skip(tys.len()) {
        type_expr(&a.value, st, true);
    }
}

/// when 覆盖收集：递归 Or 替代（`A | B -> ...` 的每个替代独立计数），
/// 兼收布尔字面量；构造器/结构体子模式无覆盖贡献
fn collect_pattern_coverage(
    p: &PatWrap,
    st: &SemState,
    covered: &mut Vec<usize>,
    covers_true: &mut bool,
    covers_false: &mut bool,
) {
    if let PatKind::Bool(v) = &p.kind {
        if *v {
            *covers_true = true;
        } else {
            *covers_false = true;
        }
    }
    if let Some(v) = st.pat_res.get(p.id).copied().and_then(|r| r.variant)
        && !covered.contains(&v)
    {
        covered.push(v);
    }
    if let PatKind::Or(ps) = &p.kind {
        for q in ps {
            collect_pattern_coverage(q, st, covered, covers_true, covers_false);
        }
    }
}
/// when 定型：主语表达式模式按 assignable 核查；
/// 穷尽性 v1 保守规则——else 兜底；枚举主语按变体覆盖核查，
/// bool 主语要求双字面量覆盖，其余主语缺 else 即 SemMissingElse
fn type_when(
    subject: Option<&ExprWrap>,
    branches: &[WhenBranch],
    span: Span,
    in_value: bool,
    st: &mut SemState,
) -> TyId {
    let subject_ty = subject.map(|s| type_expr(s, st, true));
    let mut has_else = false;
    let mut covered_variants: Vec<usize> = Vec::new();
    let mut covers_true = false;
    let mut covers_false = false;
    let mut result: Option<TyId> = None;
    for branch in branches {
        match &branch.pattern {
            WhenPattern::Expression(e) => {
                let t = type_expr(e, st, true);
                if let (Some(sty), false) = (subject_ty, st.tys.is_error(t)) {
                    // bool 表达式分支是谓词式（x > 1 -> ...），不做主语兼容核查
                    if !st.tys.is_error(sty) && t != st.tys.bool_ && !st.tys.assignable(t, sty) {
                        mismatch(st, e.span);
                    }
                }
                if let ExprKind::Bool(v) = &e.kind {
                    if *v {
                        covers_true = true;
                    } else {
                        covers_false = true;
                    }
                }
            }
            WhenPattern::Pattern(p) => {
                if let Some(sty) = subject_ty
                    && !st.tys.is_error(sty)
                {
                    refine_pattern_types(p, sty, st);
                }
                collect_pattern_coverage(
                    p,
                    st,
                    &mut covered_variants,
                    &mut covers_true,
                    &mut covers_false,
                );
            }
            WhenPattern::Else => has_else = true,
        }
        if let Some(g) = &branch.guard {
            let gt = type_expr(g, st, true);
            require_bool(gt, g.span, st);
        }
        let body_ty = match &branch.body {
            WhenBody::Expr(e) => type_expr(e, st, in_value),
            WhenBody::Block(b) => type_block(b, st, in_value),
        };
        result = Some(match result {
            None => body_ty,
            Some(prev) => st.tys.join(prev, body_ty).unwrap_or_else(|| {
                mismatch(st, span);
                st.tys.error
            }),
        });
    }
    if !has_else {
        match subject_ty {
            None => missing_else(st, span),
            Some(sty) if st.tys.is_error(sty) => {}
            Some(sty) => match st.tys.kind(sty).clone() {
                TyKind::Named(def, _) => {
                    let total = match &st.defs.get(def).kind {
                        DefKind::Enum => st.defs.get(def).sig.variants.len(),
                        _ => 0,
                    };
                    if total > 0 {
                        if covered_variants.len() < total {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemNonExhaustiveWhen,
                                span,
                                "when is not exhaustive over the enum variants",
                            ));
                        }
                    } else {
                        missing_else(st, span);
                    }
                }
                TyKind::Bool => {
                    if !(covers_true && covers_false) {
                        missing_else(st, span);
                    }
                }
                _ => missing_else(st, span),
            },
        }
    }
    result.unwrap_or(st.tys.unit)
}

fn missing_else(st: &mut SemState, span: Span) {
    st.diags.emit(Diagnostic::error(
        DiagCode::SemMissingElse,
        span,
        "when requires an else branch",
    ));
}

// ==================== 模式精化（is/in/when 主语类型下推） ====================

/// 依主语类型精化模式内绑定/构造器参数类型；
/// 单段构造器在 resolve 期未定 variant，此处按主语枚举精化补写 pat_res
fn refine_pattern_types(pat: &PatWrap, subject_ty: TyId, st: &mut SemState) {
    match &pat.kind {
        PatKind::Binding { .. } => refine_bindings(pat, subject_ty, st),
        PatKind::Wildcard | PatKind::Literal(_) | PatKind::Bool(_) | PatKind::Null => {}
        PatKind::Or(ps)
        | PatKind::Tuple { elements: ps, .. }
        | PatKind::Array { elements: ps, .. } => {
            for p in ps {
                refine_pattern_types(p, subject_ty, st);
            }
        }
        PatKind::Constructor { path, args } => {
            let resolved = st.pat_res.get(pat.id).copied().or_else(|| {
                if path.segments.len() != 1 {
                    return None;
                }
                let enum_def = match st.tys.kind(subject_ty).clone() {
                    TyKind::Named(def, _) => match st.defs.get(def).kind {
                        DefKind::Enum => Some(def),
                        _ => None,
                    },
                    _ => None,
                }?;
                let vdef = member_lookup(st, enum_def, path.segments[0].name)?;
                let variant = match st.defs.get(vdef).kind {
                    DefKind::EnumVariant { index, .. } => Some(index),
                    _ => None,
                };
                let res = PatRes {
                    ctor: vdef,
                    variant,
                };
                st.pat_res.set(pat.id, res);
                Some(res)
            });
            match resolved {
                Some(res) => {
                    let parent = match st.defs.get(res.ctor).kind {
                        DefKind::EnumVariant { parent, .. } => Some(parent),
                        _ => None,
                    };
                    if let (Some(pdef), Some(v)) = (parent, res.variant) {
                        let tys = variant_arg_tys(pdef, v, st);
                        refine_arg_patterns(args, &tys, pat.span, st);
                    }
                }
                None if path.segments.len() == 1 => {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemUndefinedName,
                        pat.span,
                        "undefined constructor",
                    ));
                }
                None => {}
            }
        }
        PatKind::Struct { fields, .. } => {
            if let Some(res) = st.pat_res.get(pat.id).copied() {
                let fs = st.defs.get(res.ctor).sig.fields.clone();
                for (name, fp) in fields {
                    if let Some((_, ft)) = fs.iter().find(|(n, _)| n == name) {
                        let ft = *ft;
                        refine_pattern_types(fp, ft, st);
                    }
                }
            }
        }
        PatKind::Range { lo, hi, .. } => {
            refine_pattern_types(lo, subject_ty, st);
            refine_pattern_types(hi, subject_ty, st);
        }
        PatKind::Error(_) | PatKind::Missing(_) => {}
    }
}

fn variant_arg_tys(enum_def: DefId, variant: usize, st: &mut SemState) -> Vec<TyId> {
    match st.defs.get(enum_def).sig.variants.get(variant) {
        Some((_, tys)) => tys.clone(),
        None => Vec::new(),
    }
}

fn refine_arg_patterns(args: &[PatWrap], tys: &[TyId], span: Span, st: &mut SemState) {
    if args.len() != tys.len() {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemArityMismatch,
            span,
            "constructor pattern arity mismatch",
        ));
    }
    for (a, t) in args.iter().zip(tys.iter()) {
        refine_pattern_types(a, *t, st);
    }
}

// ==================== 定义值类型 ====================

/// ident/path 引用处的值类型：fun → Fn(..) 型；绑定/参数 → sig.ty；
/// 类型名（builtin/struct/enum/class）→ Named（类型对象，调用处另行拒绝）；
/// 变体 → 所属枚举的 Named
fn def_value_ty(def: DefId, st: &mut SemState) -> TyId {
    match st.defs.get(def).kind.clone() {
        DefKind::Fun => {
            let sig = &st.defs.get(def).sig;
            let params: Vec<TyId> = sig.params.iter().map(|(_, t)| *t).collect();
            let ret = sig.ret;
            st.tys.intern(TyKind::Fn(params, ret))
        }
        DefKind::LocalBinding { .. } | DefKind::Param => st.defs.get(def).sig.ty,
        DefKind::BuiltinType | DefKind::Struct | DefKind::Enum | DefKind::Class => {
            st.tys.builtin_named(def)
        }
        DefKind::EnumVariant { parent, .. } => st.tys.builtin_named(parent),
        DefKind::TypeAlias => st.defs.get(def).sig.ty,
        DefKind::StructField { .. } | DefKind::ImportNs { .. } => st.tys.error,
    }
}
// ==================== debug 覆盖断言 ====================
//
// 结构镜像 check 的遍历面：check 覆盖区内每个 ExprWrap 必须已有 expr_ty。
// check 不下钻的区域（lambda/spawn/macro）同样跳过——那些子树无 expr_ty 属预期。
#[cfg(debug_assertions)]
mod verify {
    use ast::syntax::decl::{DeclKind, FunctionBody};
    use ast::syntax::expr::*;
    use ast::syntax::node::{NodeId, SourceFile};
    use ast::syntax::stmt::{StmtKind, StmtWrap};

    use crate::SemState;

    pub(super) fn run(ast: &SourceFile, st: &SemState) {
        for item in &ast.items {
            match &item.kind {
                DeclKind::Fun(f) => match &f.body {
                    Some(FunctionBody::Block(b)) => block(b, st),
                    Some(FunctionBody::Expr(e)) => expr(e, st),
                    None => {}
                },
                DeclKind::Binding(b) => {
                    if let Some(init) = &b.init {
                        expr(init, st);
                    }
                }
                _ => {}
            }
        }
    }

    fn assert_typed(id: NodeId, st: &SemState) {
        assert!(
            st.expr_ty.get(id).is_some(),
            "expr not typed (coverage drift): {id:?}"
        );
    }

    fn block(b: &Block, st: &SemState) {
        for s in &b.stmts {
            stmt(s, st);
        }
        if let Some(t) = &b.tail {
            expr(t, st);
        }
    }

    fn stmt(s: &StmtWrap, st: &SemState) {
        match &s.kind {
            StmtKind::Expr(es) => expr(&es.expr, st),
            StmtKind::Assignment { target, value, .. } => {
                expr(target, st);
                expr(value, st);
            }
            StmtKind::Binding(b) => {
                if let Some(init) = &b.init {
                    expr(init, st);
                }
            }
            StmtKind::For { iterable, body, .. } => {
                expr(iterable, st);
                block(body, st);
            }
            StmtKind::While { cond, body, .. } => {
                condition(cond, st);
                block(body, st);
            }
            StmtKind::Loop { body, .. } => block(body, st),
            StmtKind::Break { value, .. } => {
                if let Some(v) = value {
                    expr(v, st);
                }
            }
            StmtKind::Continue { .. } => {}
            StmtKind::Return { value } => {
                if let Some(v) = value {
                    expr(v, st);
                }
            }
            StmtKind::Defer { body } => block(body, st),
            StmtKind::Comptime { block: b } => block(b, st),
            StmtKind::Grow { expr: e, bits } => {
                expr(e, st);
                if let Some(b) = bits {
                    expr(b, st);
                }
            }
            StmtKind::Error(_) | StmtKind::Missing(_) => {}
        }
    }

    fn condition(c: &Condition, st: &SemState) {
        match c {
            Condition::Expr(e) => expr(e, st),
            Condition::Val { expr: e, .. } => expr(e, st),
        }
    }

    fn expr(e: &ExprWrap, st: &SemState) {
        assert_typed(e.id, st);
        match &e.kind {
            ExprKind::Binary { lhs, rhs, .. } => {
                expr(lhs, st);
                expr(rhs, st);
            }
            ExprKind::Is { lhs, .. } | ExprKind::In { lhs, .. } => expr(lhs, st),
            ExprKind::Unary { operand, .. }
            | ExprKind::ForceUnwrap { expr: operand }
            | ExprKind::Await { expr: operand }
            | ExprKind::Async { expr: operand }
            | ExprKind::Cast { expr: operand, .. }
            | ExprKind::ArrayRepeat { value: operand, .. } => expr(operand, st),
            ExprKind::Concurrent { block: b } => block(b, st),
            ExprKind::Call { callee, args, .. } => {
                expr(callee, st);
                for a in args {
                    expr(&a.value, st);
                }
            }
            ExprKind::Index { base, index } => {
                expr(base, st);
                expr(index, st);
            }
            ExprKind::Member { base, .. } => expr(base, st),
            ExprKind::StringInterp { parts } => {
                for p in parts {
                    expr(p, st);
                }
            }
            ExprKind::When { subject, branches } => {
                if let Some(s) = subject {
                    expr(s, st);
                }
                for b in branches {
                    if let WhenPattern::Expression(e) = &b.pattern {
                        expr(e, st);
                    }
                    if let Some(g) = &b.guard {
                        expr(g, st);
                    }
                    match &b.body {
                        WhenBody::Expr(e) => expr(e, st),
                        WhenBody::Block(b) => block(b, st),
                    }
                }
            }
            ExprKind::If {
                cond,
                then_block,
                else_block,
            } => {
                condition(cond, st);
                block(then_block, st);
                if let Some(eb) = else_block {
                    block(eb, st);
                }
            }
            ExprKind::Tuple(xs) | ExprKind::Array(xs) => {
                for x in xs {
                    expr(x, st);
                }
            }
            ExprKind::Comptime { block: b } | ExprKind::Unsafe { block: b } => block(b, st),
            // check 不下钻的区域（semantic-notes 有记录）
            ExprKind::Lambda { .. } | ExprKind::Spawn(_) | ExprKind::MacroCall { .. } => {}
            // 叶子与 v1 直拒节点：无表达式子节点
            _ => {}
        }
    }
}
