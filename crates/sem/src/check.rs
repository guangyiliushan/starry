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
            DeclKind::Binding(b) => check_top_binding(b, st),
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

fn check_top_binding(b: &BindingDecl, st: &mut SemState) {
    let Some(init) = &b.init else {
        // 与局部同纪律：裸绑定至少须有标注（明确赋值分析 v1 不做，见 notes）
        if b.ty.is_none() {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                b.pattern.span,
                "binding requires an initializer or a type annotation",
            ));
        }
        return;
    };
    let init_ty = type_expr(init, st, true);
    check_binding_pattern(&b.pattern, init_ty, init.span, st);
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
    check_binding_pattern(&b.pattern, init_ty, init.span, st);
}

/// 绑定模式定型核心：Binding 核对注解/写入推断，Tuple/Array 按元素递归精化；
/// 顶层与局部共用（顶层 defs 的 pat_binding_def 由 declare 期写入）
fn check_binding_pattern(pat: &PatWrap, init_ty: TyId, init_span: Span, st: &mut SemState) {
    match &pat.kind {
        PatKind::Binding { .. } => {
            if let Some(&def) = st.pat_binding_def.get(pat.id) {
                let ann = st.defs.get(def).sig.ty;
                if st.tys.needs_infer(ann) {
                    st.defs.get_mut(def).sig.ty = init_ty;
                } else if !st.tys.assignable(init_ty, ann) {
                    mismatch(st, init_span);
                }
            }
        }
        PatKind::Wildcard => {}
        PatKind::Tuple { elements, rest } | PatKind::Array { elements, rest } => {
            let elem_tys = match st.tys.kind(init_ty).clone() {
                TyKind::Tuple(ts) => Some(ts),
                TyKind::Array(t, n) => Some(vec![t; n as usize]),
                TyKind::Error | TyKind::Unknown => None,
                _ => {
                    mismatch(st, init_span);
                    None
                }
            };
            if let Some(ts) = elem_tys {
                if rest.map_or(ts.len() != elements.len(), |r| {
                    r > ts.len() || elements.len() > ts.len()
                }) {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemArityMismatch,
                        init_span,
                        "destructuring arity mismatch",
                    ));
                }
                for (i, p) in elements.iter().enumerate() {
                    let ti = match rest {
                        Some(r) if i >= *r => ts.len() - (elements.len() - i),
                        _ => i,
                    };
                    let Some(t) = ts.get(ti).copied() else {
                        continue;
                    };
                    check_binding_pattern(p, t, init_span, st);
                }
            }
        }
        _ => {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                pat.span,
                "only binding/wildcard/tuple/array patterns are supported in bindings",
            ));
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
                break_total: 0,
                break_with_value: 0,
            });
            type_block(body, st, false);
            let _ = st.loops.pop();
        }
        StmtKind::Break { value, .. } => {
            if let Some(frame) = st.loops.last_mut() {
                frame.break_total += 1;
            }
            if let Some(v) = value {
                let vty = type_expr(v, st, true);
                if let Some(frame) = st.loops.last_mut() {
                    frame.break_with_value += 1;
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
            // grow 的 lowering 未排期（irc 拒收）：sem 先行诊断
            type_expr(expr, st, true);
            if let Some(b) = bits {
                let bty = type_expr(b, st, true);
                if !st.tys.is_integral(bty) && !st.tys.is_error(bty) {
                    mismatch(st, b.span);
                }
            }
            unsupported(
                st,
                s.span,
                "grow statements are not supported in semantic v1",
            );
        }
        StmtKind::Error(_) | StmtKind::Missing(_) => {}
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
/// 成员类型查询。`diagnose` 为 false 时不发诊断、找不到返回 None
/// （type_call 的 Member callee 消歧用：找不到成员 = 方法调用，由调用点报）。
fn member_field_ty_ex(
    base_ty: TyId,
    name: Symbol,
    safe: bool,
    span: Span,
    st: &mut SemState,
    diagnose: bool,
) -> Option<TyId> {
    let mut ty = base_ty;
    // 枚举类型对象的成员访问 = 命名空间访问（`E.A` / `Opt.Some` / 变体构造路径）
    // ——解包为枚举值型；结构体静态访问（`P.x`）仍不支持
    if let TyKind::TypeObject(def) = st.tys.kind(ty).clone()
        && matches!(st.defs.get(def).kind, DefKind::Enum)
    {
        ty = st.tys.builtin_named(def);
    }
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
                    Some(st.tys.builtin_named(def))
                }
                _ => {
                    if diagnose {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnknownMember,
                            span,
                            format!("unknown variant `{}`", st.interner.resolve(name)),
                        ));
                    }
                    None
                }
            },
            DefKind::Struct => {
                let fields = st.defs.get(def).sig.fields.clone();
                match fields.iter().find(|(n, _)| *n == name) {
                    Some((_, t)) => Some(*t),
                    None => {
                        if diagnose {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemUnknownMember,
                                span,
                                format!("unknown field `{}`", st.interner.resolve(name)),
                            ));
                        }
                        None
                    }
                }
            }
            _ => {
                if diagnose {
                    unsupported(
                        st,
                        span,
                        "member access is supported on struct values only in semantic v1",
                    );
                }
                None
            }
        },
        TyKind::Error => None,
        _ => {
            if diagnose {
                unsupported(
                    st,
                    span,
                    "member access is supported on struct values only in semantic v1",
                );
            }
            None
        }
    };
    result.map(|r| {
        if nullable_base && !st.tys.is_error(r) {
            st.tys.nullable(r)
        } else {
            r
        }
    })
}

fn member_field_ty(base_ty: TyId, name: Symbol, safe: bool, span: Span, st: &mut SemState) -> TyId {
    member_field_ty_ex(base_ty, name, safe, span, st, true).unwrap_or(st.tys.error)
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
        // 值 in 集合/区间：左操作数是元素，右操作数（EBNF RangeExpr 侧）是集合
        ExprKind::In { lhs, rhs, .. } => {
            let elem_ty = type_expr(lhs, st, true);
            let coll_ty = type_expr(rhs, st, true);
            let elem = match st.tys.kind(coll_ty).clone() {
                TyKind::Array(t, _) | TyKind::Range(t) => Some(t),
                TyKind::Error => None,
                _ => {
                    mismatch(st, rhs.span);
                    None
                }
            };
            if let (Some(e), false) = (elem, st.tys.is_error(elem_ty))
                && !st.tys.is_error(e)
                && st.tys.join(elem_ty, e).is_none()
            {
                mismatch(st, lhs.span);
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
        ExprKind::Index {
            base,
            index,
            const_index,
        } => {
            let bty = type_expr(base, st, true);
            let ity = type_expr(index, st, true);
            if !st.tys.is_integral(ity) && !st.tys.is_error(ity) {
                mismatch(st, index.span);
            }
            match st.tys.kind(bty).clone() {
                TyKind::Array(t, _) => t,
                TyKind::Tuple(ts) => match const_index {
                    Some(i) if *i < ts.len() => ts[*i],
                    Some(_) => {
                        mismatch(st, index.span);
                        st.tys.error
                    }
                    None => {
                        unsupported(st, index.span, "tuple subscript must be an integer literal");
                        st.tys.error
                    }
                },
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
        ExprKind::Loop { body } => {
            st.loops.push(LoopFrame {
                break_values: Vec::new(),
                break_total: 0,
                break_with_value: 0,
            });
            let _ = type_block(body, st, false);
            let frame = st.loops.pop().expect("loop frame");
            if frame.break_total == 0 {
                // 无 break = 无出口；unit 只是保守结果型（未来 Never 类型可替换）。
                st.tys.unit
            } else {
                if frame.break_with_value != frame.break_total {
                    mismatch(st, e.span);
                }
                let mut result = frame.break_values.first().copied().unwrap_or(st.tys.unit);
                for ty in &frame.break_values[1..] {
                    result = match st.tys.join(result, *ty) {
                        Some(joined) => joined,
                        None => {
                            mismatch(st, e.span);
                            st.tys.error
                        }
                    };
                }
                result
            }
        }
        // concurrent 块的 lowering 未排期（irc 拒收）：sem 先行诊断
        ExprKind::Concurrent { block } => {
            let ty = type_block(block, st, in_value);
            unsupported(
                st,
                e.span,
                "concurrent blocks are not supported in semantic v1",
            );
            ty
        }
        // async/await 的 lowering 未排期（协程/fiber 语义未定）：sem 先行诊断
        ExprKind::Async { block } => {
            let ty = type_block(block, st, in_value);
            unsupported(st, e.span, "async blocks are not supported in semantic v1");
            ty
        }
        ExprKind::Await { expr } => {
            let ty = type_expr(expr, st, in_value);
            unsupported(st, e.span, "await is not supported in semantic v1");
            ty
        }
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
        Op::Plus => {
            // 字符串连接是 v1 唯一非数值 `+`；运行时入口与插值共用 StrConcat。
            if l == st.tys.string && r == st.tys.string {
                st.tys.string
            } else {
                match st.tys.join(l, r) {
                    Some(t) if st.tys.is_numeric(t) => t,
                    _ => {
                        mismatch(st, span);
                        st.tys.error
                    }
                }
            }
        }
        Op::Minus | Op::Star | Op::Slash | Op::Percent => match st.tys.join(l, r) {
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

fn printable_builtin_arg(ty: TyId, st: &SemState) -> bool {
    matches!(
        st.tys.kind(ty),
        TyKind::Int(_) | TyKind::Float(_) | TyKind::Bool | TyKind::String | TyKind::Char
    )
}

fn type_print_call(callee: &ExprWrap, args: &[CallArg], span: Span, st: &mut SemState) -> TyId {
    if args.len() != 1 || args.iter().any(|a| a.name.is_some()) {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemArityMismatch,
            span,
            "print expects one positional argument",
        ));
        type_all_args(args, st);
        st.expr_ty.set(callee.id, st.tys.unit);
        return st.tys.unit;
    }
    let arg_ty = type_expr(&args[0].value, st, true);
    if !st.tys.is_error(arg_ty) && !printable_builtin_arg(arg_ty, st) {
        unsupported(
            st,
            args[0].value.span,
            "print supports scalar int/float/bool/char/string arguments in semantic v1",
        );
    }
    st.expr_ty.set(callee.id, st.tys.unit);
    st.tys.unit
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
    if let Some(def) = st.expr_def.get(callee.id).copied()
        && matches!(st.defs.get(def).kind, DefKind::BuiltinPrint)
    {
        return type_print_call(callee, args, span, st);
    }
    // Member 目标的定型与消歧一次完成：字段（含函数字段）/枚举变体查得到，
    // 查不到即方法调用（由此处报「不支持」，不让 member_field_ty 发 unknown-field 误导）
    let callee_ty = match &callee.kind {
        ExprKind::Member { base, name, safe } => {
            let base_ty = type_expr(base, st, true);
            let fty = if st.tys.is_error(base_ty) {
                st.tys.error
            } else {
                match member_field_ty_ex(base_ty, *name, *safe, callee.span, st, false) {
                    Some(t) => t,
                    None => {
                        unsupported(
                            st,
                            callee.span,
                            "method calls are not supported in semantic v1",
                        );
                        st.tys.error
                    }
                }
            };
            st.expr_ty.set(callee.id, fty);
            fty
        }
        _ => type_expr(callee, st, true),
    };
    if !type_args.is_empty() {
        unsupported(
            st,
            span,
            "generic call instantiation is not supported in semantic v1",
        );
    }
    // callee 已出错（未定义名/错误基对象等，诊断已发）：不再级联第二条
    if st.tys.is_error(callee_ty) {
        type_all_args(args, st);
        return st.tys.error;
    }
    // 构造器调用：
    // - Named(enum)：变体成员访问（`Opt.Some(...)`，member_field_ty 给枚举型）
    // - TypeObject：类型名的值位置出现（`P(...)`；`E(...)` 不是构造，报错）
    match st.tys.kind(callee_ty).clone() {
        TyKind::Named(named, _) => {
            if matches!(st.defs.get(named).kind, DefKind::Enum)
                && let ExprKind::Member { name, .. } = &callee.kind
            {
                let resolved =
                    member_lookup(st, named, *name).and_then(|v| match st.defs.get(v).kind {
                        DefKind::EnumVariant { parent, index } => Some((parent, index)),
                        _ => None,
                    });
                if let Some((parent, index)) = resolved {
                    let vparams = variant_arg_tys(parent, index, st);
                    check_call_args(args, &vparams, &[], span, st);
                } else {
                    type_all_args(args, st);
                }
                return st.tys.builtin_named(named);
            }
        }
        TyKind::TypeObject(named) => match st.defs.get(named).kind.clone() {
            DefKind::Struct => {
                let fields = st.defs.get(named).sig.fields.clone();
                check_call_args(args, &fields, &[], span, st);
                return st.tys.builtin_named(named);
            }
            DefKind::Enum => {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemNotCallable,
                    callee.span,
                    "enum type name is not callable; use a variant constructor",
                ));
                type_all_args(args, st);
                return st.tys.error;
            }
            // 别名：经预解析目标走到目标的构造器（type Q = P; Q(1)）
            DefKind::TypeAlias => {
                let target = st.defs.get(named).alias_resolved;
                if let Some(t) = target
                    && matches!(st.defs.get(t).kind, DefKind::Struct)
                {
                    let fields = st.defs.get(t).sig.fields.clone();
                    check_call_args(args, &fields, &[], span, st);
                    return st.tys.builtin_named(t);
                }
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemNotCallable,
                    callee.span,
                    "aliased type is not callable in semantic v1",
                ));
                type_all_args(args, st);
                return st.tys.error;
            }
            _ => {}
        },
        _ => {}
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
    if let Some(def) = st.expr_def.get(callee.id).copied() {
        // class 名：声明点已报 unsupported，此处静默兑现「注册避免级联」
        if !matches!(st.defs.get(def).kind, DefKind::Class) {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemNotCallable,
                callee.span,
                "callee is not a function",
            ));
        }
        type_all_args(args, st);
        return st.tys.error;
    }
    // 落到此处的 Member 目标 = 字段存在但非函数类型（真方法调用已在定型处诊断）
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
    // 命名实参名不存在时已报错：不再级联「missing argument」（一条错误一条诊断）
    let mut saw_unknown_named = false;
    for arg in args {
        let idx = match arg.name {
            Some(name) => match params.iter().position(|(n, _)| *n == name) {
                Some(i) => i,
                None => {
                    saw_unknown_named = true;
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemUnknownParam,
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
                if !saw_unknown_named {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemArityMismatch,
                        span,
                        format!(
                            "missing argument for parameter `{}`",
                            st.interner.resolve(params[i].0)
                        ),
                    ));
                }
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

/// 不可驳模式判定：通配/绑定恒不可驳；元组/数组/结构体的元素全不可驳则
/// 不可驳；Or 含任一不可驳替代即不可驳（其语义即替代的全覆盖）。
/// 构造器/字面量/区间/布尔恒可驳。
fn is_irrefutable(pat: &PatWrap) -> bool {
    match &pat.kind {
        PatKind::Wildcard | PatKind::Binding { .. } => true,
        PatKind::Tuple { elements, .. } | PatKind::Array { elements, .. } => {
            elements.iter().all(is_irrefutable)
        }
        PatKind::Struct { fields, .. } => fields.iter().all(|(_, p)| is_irrefutable(p)),
        PatKind::Or(ps) => ps.iter().any(is_irrefutable),
        _ => false,
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
    // 不可驳模式（通配/绑定、全解构元组/数组/结构体、含不可驳替代的 Or）
    // 即穷尽，不再要求 else（`when x { _ -> 0 }` 的最直白假阳性）
    let irrefutable_seen = branches
        .iter()
        .any(|b| matches!(&b.pattern, WhenPattern::Pattern(p) if is_irrefutable(p)));
    if !has_else && !irrefutable_seen {
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
        // 或模式各替代同型：逐个按主语精化
        PatKind::Or(ps) => {
            for p in ps {
                refine_pattern_types(p, subject_ty, st);
            }
        }
        // 元组/数组模式按元素类型解构精化（when 主语为同型聚合时）；
        // `..` 不占元素名，其后元素从主语尾部倒序映射
        PatKind::Tuple { elements, rest } | PatKind::Array { elements, rest } => {
            let elem_tys: Option<Vec<TyId>> = match st.tys.kind(subject_ty).clone() {
                TyKind::Tuple(ts) => Some(ts),
                TyKind::Array(t, n) => Some(vec![t; n as usize]),
                _ => None,
            };
            match elem_tys {
                Some(ts) => {
                    for (i, p) in elements.iter().enumerate() {
                        let ti = match rest {
                            Some(r) if i >= *r => ts.len().saturating_sub(elements.len() - i),
                            _ => i,
                        };
                        if let Some(t) = ts.get(ti).copied() {
                            refine_pattern_types(p, t, st);
                        }
                    }
                }
                None => {
                    for p in elements {
                        refine_pattern_types(p, subject_ty, st);
                    }
                }
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
                        let tys: Vec<TyId> = variant_arg_tys(pdef, v, st)
                            .iter()
                            .map(|(_, t)| *t)
                            .collect();
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

/// 变体形参（名, 类型）：名保留以支持命名实参构造
fn variant_arg_tys(enum_def: DefId, variant: usize, st: &mut SemState) -> Vec<(Symbol, TyId)> {
    match st.defs.get(enum_def).sig.variants.get(variant) {
        Some((_, args)) => args.clone(),
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
        // 类型名在值位置出现：类型对象（TypeObject），与「该类型的值」分立——
        // 仅 type_call 的构造器分支消费它，其余使用点不可指派/不可调用
        DefKind::BuiltinType | DefKind::Struct | DefKind::Enum | DefKind::Class => {
            st.tys.intern(TyKind::TypeObject(def))
        }
        DefKind::EnumVariant { parent, .. } => st.tys.builtin_named(parent),
        // 别名在值位置归一到目标：TypeObject(alias_resolved)，
        // 成员访问/模式路径/构造经目标 DefId 统一解析（type_call 的
        // TypeAlias 臂仅兜未 lower 的别名）
        DefKind::TypeAlias => {
            let target = st.defs.get(def).alias_resolved.unwrap_or(def);
            st.tys.intern(TyKind::TypeObject(target))
        }
        DefKind::StructField { .. } | DefKind::ImportNs { .. } | DefKind::BuiltinPrint => {
            st.tys.error
        }
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
            ExprKind::Is { lhs, .. } => expr(lhs, st),
            ExprKind::In { lhs, rhs, .. } => {
                expr(lhs, st);
                expr(rhs, st);
            }
            ExprKind::Unary { operand, .. }
            | ExprKind::ForceUnwrap { expr: operand }
            | ExprKind::Await { expr: operand }
            | ExprKind::Cast { expr: operand, .. }
            | ExprKind::ArrayRepeat { value: operand, .. } => expr(operand, st),
            ExprKind::Async { block: b } | ExprKind::Concurrent { block: b } => block(b, st),
            ExprKind::Call { callee, args, .. } => {
                expr(callee, st);
                for a in args {
                    expr(&a.value, st);
                }
            }
            ExprKind::Index { base, index, .. } => {
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
