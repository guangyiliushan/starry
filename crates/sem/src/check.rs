//! Pass 3：check —— 自底向上定型，写 expr_ty；`Error` 类型投毒抑制级联
//!
//! 教科书依据：龙书 6.5（类型检查）+ Tiger ch.5（环境 + 定型规则）。
//! 提升格规则见 semantic-notes（整型加宽 / int→float / Null→Nullable）。

use ast::interner::Symbol;
use ast::syntax::decl::{BindingDecl, DeclKind, FunctionBody, TraitMember};
use ast::syntax::expr::*;
use ast::syntax::node::{DiagCode, Diagnostic, NodeId, SourceFile};
use ast::syntax::pat::{PatKind, PatWrap};
use ast::syntax::stmt::{StmtKind, StmtWrap};
use ast::syntax::ty::TypeArg;
use ast::token::{LiteralKind, OperatorKind, Span};

use crate::declare::member_lookup;
use crate::def::{DefId, DefKind};
use crate::ty::{IntKind, TyId, TyKind};
use crate::{LoopFrame, PatRes, SemState};

/// Regular lattice assignment plus the Phase 2a class-root -> dyn unsize rule.
fn assignable_with_unsize(st: &mut SemState, from: TyId, to: TyId) -> bool {
    if st.tys.assignable(from, to) {
        return true;
    }
    // Unwrap ?dyn and optional class roots, then require a monomorphic
    // trait and a concrete class payload.
    let to_inner = match st.tys.kind(to) {
        TyKind::Dyn(..) => to,
        TyKind::Nullable(inner) if matches!(st.tys.kind(*inner), TyKind::Dyn(..)) => *inner,
        _ => return false,
    };
    let from_inner = match st.tys.kind(from) {
        TyKind::Nullable(inner) => *inner,
        _ => from,
    };
    let TyKind::Dyn(trait_def, trait_args) = st.tys.kind(to_inner) else {
        return false;
    };
    let TyKind::Named(type_def, type_args) = st.tys.kind(from_inner) else {
        return false;
    };
    let target = *trait_def;
    matches!(st.defs.get(*type_def).kind, DefKind::Class)
        && trait_args.is_empty()
        && type_args.is_empty()
        && st.defs.get(target).ty_params == 0
}
pub(crate) fn run(ast: &SourceFile, st: &mut SemState) {
    run_from(ast, st, 0);
    let root = st.scopes.root();
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
    check_recursive_types(ast, st);
    #[cfg(debug_assertions)]
    verify::run(ast, st);
}

/// M7 增量入口：只处理克隆追加的 items[start..]。
/// 收尾推断/递归类型检测/verify 只服务原件（克隆是 fun，不引入绑定/struct）。
pub(crate) fn run_from(ast: &SourceFile, st: &mut SemState, start: usize) {
    for (i, item) in ast.items.iter().enumerate().skip(start) {
        match &item.kind {
            DeclKind::Fun(f) => {
                let Some(fdef) = st.decl_def.get(i).copied().flatten() else {
                    continue;
                };
                check_fun(item.id, f, fdef, st)
            }
            // M4 slice 2b：class 成员函数/deinit 体检查（self 已由 resolve 期绑定）
            DeclKind::Class(c) => {
                let Some(class_def) = st.decl_def.get(i).copied().flatten() else {
                    continue;
                };
                for member in &c.members {
                    match member {
                        ast::syntax::decl::ClassMember::Declaration(d)
                            if matches!(&d.kind, DeclKind::Fun(_)) =>
                        {
                            if let DeclKind::Fun(f) = &d.kind {
                                let Some(mdef) = member_lookup(st, class_def, f.name) else {
                                    continue;
                                };
                                check_fun(d.id, f, mdef, st);
                            }
                        }
                        ast::syntax::decl::ClassMember::Deinit(b) => {
                            let dname = st.interner.intern("deinit");
                            let Some(mdef) = member_lookup(st, class_def, dname) else {
                                continue;
                            };
                            let anchor_id = b.stmts.first().map(|s| s.id).unwrap_or(NodeId(0));
                            st.funs.push(crate::FunFrame {
                                node: anchor_id,
                                def: mdef,
                            });
                            let ty = type_block(b, st, true);
                            if !assignable_with_unsize(st, ty, st.tys.unit) {
                                mismatch(
                                    st,
                                    b.stmts
                                        .first()
                                        .map(|s| s.span)
                                        .unwrap_or(Span { start: 0, end: 0 }),
                                );
                            }
                            st.funs.pop();
                        }
                        _ => {}
                    }
                }
            }
            // WP5: trait default bodies are checked against the trait Self
            // placeholder; impl dispatch reuses the resulting method def.
            DeclKind::Trait(t) => {
                let Some(tdef) = st.decl_def.get(i).copied().flatten() else {
                    continue;
                };
                let Some(ms) = st.defs.get(tdef).member_scope else {
                    continue;
                };
                let self_key = st.interner.intern("Self");
                let Some(self_def) = st.scopes.lookup_local(ms, self_key) else {
                    continue;
                };
                let _self_ty = st.tys.builtin_named(self_def);
                for member in &t.members {
                    if let TraitMember::Fun(f) = member
                        && f.body.is_some()
                        && let Some(mdef) = member_lookup(st, tdef, f.name)
                    {
                        check_fun(item.id, f, mdef, st);
                    }
                }
            }
            DeclKind::Struct(s) => {
                // M3：主构造默认值逐参数定型（上下文=参数类型，含字面量重定型）。
                // M7：泛型 struct 默认值在 declare 期已拒收，这里跳过（无逐实例化检查）
                if !s.type_params.is_empty() {
                    continue;
                }
                for p in &s.primary_ctor {
                    let Some(d) = &p.default else { continue };
                    let dty = type_expr(d, st, true);
                    if let Some(t) = &p.ty {
                        let target = st.ty_of_node.get(t.id).copied().unwrap_or(st.tys.error);
                        retype_binary_literals(d, target, st);
                        retype_integer_literal(d, target, st);

                        let dty = st.expr_ty.get(d.id).copied().unwrap_or(dty);
                        if !assignable_with_unsize(st, dty, target) {
                            mismatch(st, d.span);
                        }
                    }
                }
            }
            // M9 9b：impl 方法体核查（覆盖纪律；self 类型经 resolve 已注入 sig.ty）
            DeclKind::Impl(i) => {
                let Some(entry) = st.impls.iter().find(|im| im.span == item.span).cloned() else {
                    continue;
                };
                for member in &i.members {
                    if let ast::syntax::decl::ClassMember::Declaration(d) = member
                        && let DeclKind::Fun(f) = &d.kind
                        && let Some((_, mdef)) = entry.methods.iter().find(|(n, _)| *n == f.name)
                    {
                        check_fun(d.id, f, *mdef, st);
                    }
                }
            }
            DeclKind::Binding(b) => check_top_binding(b, st),
            _ => {}
        }
    }
}

/// 递归类型检测：按值容纳自身的 struct/enum 是无限大小（codegen 会
/// 在 scalar_slots 上栈溢出）。别名循环已由 lower 期 SemTypeCycle 覆盖，
/// 这里补聚合递归：字段/变体 payload/元组/数组/nullable 都按值展开。
fn check_recursive_types(ast: &SourceFile, st: &mut SemState) {
    use std::collections::HashMap;
    let root = st.scopes.root();
    // 声明序 → DefId（struct/enum 只在根作用域，v1 无嵌套声明）
    let mut decl_of: HashMap<usize, usize> = HashMap::new();
    for item in &ast.items {
        let (name, span) = match &item.kind {
            DeclKind::Struct(s) => (s.name, item.span),
            DeclKind::Enum(e) => (e.name, item.span),
            _ => continue,
        };
        if let Some(def) = st.scopes.lookup_local(root, name) {
            decl_of.insert(def.0 as usize, span.start as usize);
        }
    }
    if decl_of.is_empty() {
        return;
    }
    let n = st.defs.len();
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); n];
    for &def_idx in decl_of.keys() {
        let def = DefId(def_idx as u32);
        let mut refs = Vec::new();
        match st.defs.get(def).kind {
            DefKind::Struct | DefKind::Class => {
                let fields = st.defs.get(def).sig.fields.clone();
                for (_, ty) in fields {
                    collect_named_refs(st, ty, &mut refs);
                }
            }
            DefKind::Enum => {
                let variants = st.defs.get(def).sig.variants.clone();
                for (_, params) in variants {
                    for (_, ty) in params {
                        collect_named_refs(st, ty, &mut refs);
                    }
                }
            }
            _ => {}
        }
        edges[def_idx] = refs
            .into_iter()
            .filter(|d| decl_of.contains_key(&(d.0 as usize)))
            .map(|d| d.0 as usize)
            .collect();
    }
    // 按声明源序迭代（HashMap key 序随进程随机，诊断序必须确定）
    let mut decls: Vec<usize> = decl_of.keys().copied().collect();
    decls.sort_unstable();
    let mut state = vec![0u8; n];
    let mut reported = vec![false; n];
    for start in decls {
        if state[start] != 0 {
            continue;
        }
        // 显式栈迭代 DFS：声明数不受 parser 深度限制，深链递归会爆栈
        state[start] = 1;
        let mut stack: Vec<(usize, usize)> = vec![(start, 0)];
        while let Some(top) = stack.last_mut() {
            let node = top.0;
            if top.1 < edges[node].len() {
                let next = edges[node][top.1];
                top.1 += 1;
                match state[next] {
                    0 => {
                        state[next] = 1;
                        stack.push((next, 0));
                    }
                    1 if !reported[next] => {
                        reported[next] = true;
                        if let Some(&s) = decl_of.get(&next) {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemTypeCycle,
                                Span {
                                    start: s as u32,
                                    end: s as u32,
                                },
                                "recursive type has infinite size (store it behind a nullable/indirection boundary is not yet supported)",
                            ));
                        }
                    }
                    _ => {}
                }
            } else {
                state[node] = 2;
                stack.pop();
            }
        }
    }
}

fn collect_named_refs(st: &SemState, ty: TyId, out: &mut Vec<DefId>) {
    match st.tys.kind(ty).clone() {
        TyKind::Named(def, args) => {
            out.push(def);
            for a in args {
                collect_named_refs(st, a, out);
            }
        }
        TyKind::Nullable(inner) | TyKind::Array(inner, _) | TyKind::Range(inner) => {
            collect_named_refs(st, inner, out)
        }
        TyKind::Tuple(es) => {
            for e in es {
                collect_named_refs(st, e, out);
            }
        }
        _ => {}
    }
}

fn check_fun(decl_id: NodeId, f: &ast::syntax::decl::FunDecl, fdef: DefId, st: &mut SemState) {
    // M7：泛型原件不检查（无 bounds 可依；检查发生在实例化克隆上，
    // 见 semantic-notes「泛型单态化」）。克隆 AST 仍带 type_params，
    // 区分靠 def：spec def 的 ty_params = 0。拒收面诊断在 declare 期已钉。
    if st.defs.get(fdef).ty_params > 0 {
        return;
    }
    let ret = st.defs.get(fdef).sig.ret;
    // M4 slice 4c: class/?P params and returns are scheduled (+0/+1).
    // Params are borrowed (+0, caller keeps ownership); a callee
    // binding retains explicitly. Returns are owned (+1): the call-site
    // binding takes over the release (call-init skip-retain rule).
    // M2 非逃逸：返回借用 = 悬垂，v1 拒绝
    if matches!(st.tys.kind(ret), TyKind::Ref { .. }) {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemUnsupportedConstruct,
            f.ret
                .as_ref()
                .map(|t| t.span)
                .unwrap_or(Span { start: 0, end: 0 }),
            "returning a reference would dangle; not supported in semantic v1",
        ));
    }
    // 默认实参：逐参数型核查（irc 在调用点重放 AST，必须已有 expr_ty）。
    // 默认值表达式在 resolve 期已绑定；此处恰好定型一次。
    let param_tys = st.defs.get(fdef).sig.params.clone();
    for (i, p) in f.params.iter().enumerate() {
        let Some(default) = &p.default else {
            continue;
        };
        let dty = type_expr(default, st, true);
        if let Some((_, pty)) = param_tys.get(i)
            && !assignable_with_unsize(st, dty, *pty)
        {
            mismatch(st, default.span);
        }
    }
    st.funs.push(crate::FunFrame {
        node: decl_id,
        def: fdef,
    });
    let tail_ty = match &f.body {
        Some(FunctionBody::Block(b)) => {
            let ty = type_block(b, st, true);
            // M1：尾式字面量按声明返回型重定型（值域按目标宽度诊断）
            if f.ret.is_some()
                && let Some(t) = &b.tail
            {
                retype_binary_literals(t, ret, st);
                retype_integer_literal(t, ret, st);
            }
            b.tail.as_ref().map(|_| ty)
        }
        Some(FunctionBody::Expr(e)) => {
            let ty = type_expr(e, st, true);
            if f.ret.is_some() {
                retype_binary_literals(e, ret, st);
                retype_integer_literal(e, ret, st);
            }
            Some(st.expr_ty.get(e.id).copied().unwrap_or(ty))
        }
        None => None,
    };
    // 返回经由 return 语句逐点核查；块体无尾表达式时不比较。
    // 仅声明了返回型才与尾表达式比较：未声明返回型按 unit 推断，
    // 尾表达式求值但不核查——`fun main() { helper() }` 是惯用形状，
    // 不能假阳性；`: unit` 与未声明的差异记入 semantic-notes
    if f.ret.is_some()
        && let Some(body_ty) = tail_ty
        && !assignable_with_unsize(st, body_ty, ret)
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
    // S2/P2-3b：绑定位置 lambda 的期望 Fn 类型（注解驱动）——与
    // check_call_args 的实参压栈同纪律；仅 Lambda init 压栈（配对弹出）
    if matches!(&init.kind, ExprKind::Lambda { .. })
        && let Some(&def) = st.pat_binding_def.get(b.pattern.id)
    {
        let ann = st.defs.get(def).sig.ty;
        if matches!(st.tys.kind(ann), TyKind::Fn(..)) {
            st.lambda_exp.push(Some(ann));
        }
    }
    let init_ty = type_expr(init, st, true);
    // S2/P2-3b-2：顶层 Fn 绑定 = 全局逃逸（无全局 release 站点）——v1 拒收
    if matches!(st.tys.kind(init_ty), TyKind::Fn(..)) {
        unsupported(
            st,
            init.span,
            "top-level Fn bindings are not supported in v1 (no global release site)",
        );
    }
    check_binding_pattern(init, &b.pattern, init_ty, st);
}

fn check_local_binding(b: &BindingDecl, st: &mut SemState) {
    let weak = b
        .modifiers
        .modifiers
        .iter()
        .any(|m| m.word == ast::token::ModifierWord::Weak);
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
    // S2/P2-3b：绑定位置 lambda 的期望 Fn 类型（注解驱动）——与
    // check_call_args 的实参压栈同纪律；仅 Lambda init 压栈（配对弹出）
    if matches!(&init.kind, ExprKind::Lambda { .. })
        && let Some(&def) = st.pat_binding_def.get(b.pattern.id)
    {
        let ann = st.defs.get(def).sig.ty;
        if matches!(st.tys.kind(ann), TyKind::Fn(..)) {
            st.lambda_exp.push(Some(ann));
        }
    }
    let init_ty = type_expr(init, st, true);
    // M4 slice 3：weak 绑定只接受 class 值（弱引用语义）
    if weak && !crate::declare::is_class_ty(st, init_ty) {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemTypeMismatch,
            init.span,
            "`weak` bindings require a class value initializer",
        ));
    }
    check_binding_pattern(init, &b.pattern, init_ty, st);
}

/// 绑定模式定型核心：Binding 核对注解/写入推断，Tuple/Array 按元素递归精化；
/// 顶层与局部共用（顶层 defs 的 pat_binding_def 由 declare 期写入）
/// M7 slice 2：泛型 enum 的零参变体构造（`Opt.None`）定型为泛型原件
/// `Named(orig, [])`；标注/目标是实例化 spec（同名的 `Named(spec,[])`）
/// 时视为可指派（零参变体布局与 T 无关）
fn enum_ctor_matches_spec(st: &SemState, from: TyId, to: TyId) -> bool {
    match (st.tys.kind(from).clone(), st.tys.kind(to).clone()) {
        (TyKind::Named(fd, fa), TyKind::Named(td, ta)) => {
            fa.is_empty()
                && ta.is_empty()
                && fd != td
                && st.defs.get(fd).name == st.defs.get(td).name
                && matches!(st.defs.get(fd).kind, DefKind::Enum)
                && matches!(st.defs.get(td).kind, DefKind::Enum)
                && st.defs.get(td).ty_params == 0
                && st.mono_insts.iter().any(|(o, s)| o.0 == fd && *s == td)
        }
        _ => false,
    }
}

fn check_binding_pattern(init: &ExprWrap, pat: &PatWrap, init_ty: TyId, st: &mut SemState) {
    match &pat.kind {
        PatKind::Binding { .. } => {
            if let Some(&def) = st.pat_binding_def.get(pat.id) {
                let ann = st.defs.get(def).sig.ty;
                if st.tys.needs_infer(ann) {
                    st.defs.get_mut(def).sig.ty = init_ty;
                } else {
                    // M1：标注是具体整型且 init 是整数字面量 → 按标注宽度重定型
                    retype_binary_literals(init, ann, st);
                    retype_integer_literal(init, ann, st);
                    let init_ty = st.expr_ty.get(init.id).copied().unwrap_or(init_ty);
                    // M7 slice 2：泛型 enum 零参 ctor → spec 豁免（见 helper）
                    if !assignable_with_unsize(st, init_ty, ann)
                        && !enum_ctor_matches_spec(st, init_ty, ann)
                    {
                        mismatch(st, init.span);
                    }
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
                    mismatch(st, init.span);
                    None
                }
            };
            if let Some(ts) = elem_tys {
                if rest.map_or(ts.len() != elements.len(), |r| {
                    r > ts.len() || elements.len() > ts.len()
                }) {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemArityMismatch,
                        init.span,
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
                    check_binding_pattern(init, p, t, st);
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
                    if !assignable_with_unsize(st, value_ty, target_ty) {
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
                    let ok = assignable_with_unsize(st, value_ty, target_ty)
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
                refine_pattern_types(pattern, t, st);
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
                // M1：return 字面量按返回型重定型
                retype_binary_literals(v, ret, st);
                retype_integer_literal(v, ret, st);
                let vty = st.expr_ty.get(v.id).copied().unwrap_or(vty);
                if !assignable_with_unsize(st, vty, ret) {
                    mismatch(st, v.span);
                }
            } else if !assignable_with_unsize(st, st.tys.unit, ret) {
                // 裸 return 等价返回 unit：非 unit 返回型报错
                mismatch(st, s.span);
            }
        }
        StmtKind::Defer { body } => {
            type_block(body, st, false);
        }
        StmtKind::Comptime { block } => {
            // P0 compiler trust：comptime 无编译期求值实现——此前按普通块
            // 在运行时求值（语义谎言）；稳定拒收后仍定型块体防级联。
            unsupported(st, s.span, "comptime blocks are not supported in v1");
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
                DefKind::LocalBinding { mutable, weak } => {
                    // S2/P2-3：lambda 内对捕获名赋值 = 值捕获快照语义禁止
                    if let Some((_, _, caps)) = st.lambda_cap_stack.last()
                        && caps.iter().any(|(s, _)| st.defs.get(def).name == *s)
                    {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            target.span,
                            "cannot assign to a captured variable (S2 value capture snapshot)",
                        ));
                    }
                    // M4 slice 3：weak 重赋值簿记（旧 weak_drop/新 weak_retain）未排期
                    if *weak {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            target.span,
                            "reassigning a `weak` binding is not supported in this slice",
                        ));
                    } else if !mutable {
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
        ExprKind::Deref { expr } => {
            let pty = st.expr_ty.get(expr.id).copied().unwrap_or(st.tys.error);
            require_unsafe_if_raw_deref(st, pty, target.span);
            // M2 经解引赋值：要求可变形（*mut / &mut），否则按不可写拒绝
            match st.tys.kind(pty).clone() {
                TyKind::Ref { mutable, .. } | TyKind::Pointer { mutable, .. } if !mutable => {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemAssignToVal,
                        target.span,
                        "cannot assign through an immutable reference/pointer",
                    ));
                }
                _ => {}
            }
            target_ty
        }
        ExprKind::Member { base, name, .. } => {
            // M3 字段三态：Some(true)=var 恒可写；Some(false)=val 恒拒；
            // None=legacy 按根绑定穿透检查（兼容无修饰符 body 字段）。
            let bty = st.expr_ty.get(base.id).copied().unwrap_or(st.tys.error);
            let mut checked = false;
            if let TyKind::Named(d, _) = st.tys.kind(bty)
                && let Some(fdef) = member_lookup(st, *d, *name)
                && let DefKind::StructField { mutable, .. } = &st.defs.get(fdef).kind
            {
                checked = true;
                match mutable {
                    Some(true) => {}
                    Some(false) => {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemAssignToVal,
                            target.span,
                            "cannot assign to a `val` field",
                        ));
                    }
                    None => check_root_mutability(target, st),
                }
            }
            if !checked {
                check_root_mutability(target, st);
            }
            target_ty
        }
        ExprKind::Index { .. } => {
            check_root_mutability(target, st);
            target_ty
        }
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

/// legacy 根穿透检查：val 绑定与参数的字段/元素赋值不可写（None 态字段用）
fn check_root_mutability(target: &ExprWrap, st: &mut SemState) {
    if let Some(root) = root_ident(target)
        && let Some(def) = st.expr_def.get(root.id).copied()
    {
        match &st.defs.get(def).kind {
            // M4 slice 3：weak 绑定不可重赋值（旧值 weak_drop/新值 weak_retain
            // 的簿记未排期）
            DefKind::LocalBinding { weak: true, .. } => {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnsupportedConstruct,
                    target.span,
                    "reassigning a `weak` binding is not supported in this slice",
                ));
            }
            DefKind::LocalBinding { mutable: false, .. } => {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemAssignToVal,
                    target.span,
                    "cannot assign to a field/element of an immutable binding",
                ));
            }
            DefKind::Param => {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemAssignToVal,
                    target.span,
                    "parameters are immutable in semantic v1",
                ));
            }
            _ => {}
        }
    }
}

/// 赋值目标的最内层基座标识符（`p.x` / `a[i]` / `p.xs[0]` → 根 Ident）
fn root_ident(e: &ExprWrap) -> Option<&ExprWrap> {
    match &e.kind {
        ExprKind::Member { base, .. } | ExprKind::Index { base, .. } => root_ident(base),
        ExprKind::Ident(_) => Some(e),
        _ => None,
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
            DefKind::Struct | DefKind::Class => {
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
            // 与 when/is 共用同一套主语下推：构造器模式补 pat_res，
            // 元组/数组按元素类型递归精化（旧 refine_bindings 只覆盖裸绑定）。
            refine_pattern_types(pattern, ty, st);
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

/// Enum payload layout is tag-switched but not yet ARC-safe; reject only
fn require_unsafe_if_raw_deref(st: &mut SemState, ty: TyId, span: Span) {
    if st.unsafe_depth == 0 && matches!(st.tys.kind(ty), TyKind::Pointer { .. }) {
        // Assignment targets are typed as expressions first; the shared
        // assignment gate must be idempotent for that same dereference.
        if st
            .diags
            .find_unique(span, DiagCode::SemUnsafeRequired)
            .is_some()
        {
            return;
        }
        st.diags.emit(Diagnostic::error(
            DiagCode::SemUnsafeRequired,
            span,
            "raw pointer dereference requires an unsafe block",
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

/// 整数字面量定型（M1）：未约束默认 i64（超出取 i128，再超报
/// IntegerOutOfRange）；约束上下文（标注/形参/返回）由
/// `retype_integer_literal` 按目标宽度重定型并做值域诊断。
fn integer_literal_ty(e: &ExprWrap, st: &mut SemState) -> TyId {
    let Some(v) = parse_integer_literal(e.span.slice(&st.text)) else {
        st.diags.emit(Diagnostic::error(
            DiagCode::IntegerOutOfRange,
            e.span,
            "integer literal out of range",
        ));
        return st.tys.error;
    };
    let kind = if fits_signed(v, 64) {
        IntKind::I64
    } else {
        IntKind::I128
    };
    st.tys.intern(TyKind::Int(kind))
}

/// M1 字面量上下文定型：e 是整数字面量、target 是具体整型时，按目标宽度
/// 重定型（值域按目标宽度诊断，越界报 IntegerOutOfRange 并投毒）。
/// M2 T1 借用检查（v1 保守）：目标限局部名（Ident）；顶层全局
/// 逃逸拒绝；&mut 要求可变绑定。非逃逸（不存字段/全局/返回）由
/// 类型层保证：引用只与同一 Ref 类型等价（格无聚合臂），struct 字段
/// 声明 &T 在 declare 期拒绝。
fn check_borrow(target: &ExprWrap, mutable: bool, st: &mut SemState) {
    let ExprKind::Ident(name) = &target.kind else {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemUnsupportedConstruct,
            target.span,
            "borrow targets other than a plain local name are not supported in semantic v1",
        ));
        return;
    };
    let Some(def) = st.expr_def.get(target.id).copied() else {
        return;
    };
    // 顶层全局借用 = 逃逸（跨函数生命周期），v1 拒绝
    if st.defs.get(def).scope == st.scopes.root() {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemUnsupportedConstruct,
            target.span,
            "borrowing a top-level global escapes; not supported in semantic v1",
        ));
        return;
    }
    if mutable {
        match &st.defs.get(def).kind {
            DefKind::LocalBinding { mutable: true, .. } => {}
            // M4 slice 3：weak 绑定不可重赋值（旧值 weak_drop/新值 weak_retain
            // 的簿记未排期）
            DefKind::LocalBinding { weak: true, .. } => {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnsupportedConstruct,
                    target.span,
                    "reassigning a `weak` binding is not supported in this slice",
                ));
            }
            DefKind::LocalBinding { mutable: false, .. } => {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemAssignToVal,
                    target.span,
                    "cannot borrow an immutable binding as mutable",
                ));
            }
            _ => {}
        }
    }
    let _ = name;
}

/// 调用点：绑定标注、构造/函数实参槽、return/尾式。已是 Error 的不再动。
/// M1（D3）：二元字面量对的上下文传播——`val x: i32 = 2147483647 + 1` 这类
/// 两端皆整数字面量的算术式，先把操作数重定型到期望类型再定型二元式
/// （否则默认 i64 先 join，标注/返回/实参上下文够不到，D3 裁定的缺口）。
/// 在每一处 retype_integer_literal 的调用点之前调用。
/// 范围：操作数须为裸整数字面量——一元负（`-5 + 1`）与嵌套链不传播
/// （记入差距：D3 裁定只要求字面量对）。
fn retype_binary_literals(e: &ExprWrap, target: TyId, st: &mut SemState) {
    if let ExprKind::Binary { op, lhs, rhs } = &e.kind
        && matches!(
            op,
            OperatorKind::Plus
                | OperatorKind::Minus
                | OperatorKind::Star
                | OperatorKind::Slash
                | OperatorKind::Percent
        )
    {
        retype_integer_literal(lhs, target, st);
        retype_integer_literal(rhs, target, st);
        // 重定型二元节点本身：不递归下钻（下钻会把字面量定型回默认宽
        // 度，抹掉上面的重定型）；只按操作数现型汇合 + 常量核查
        let l = st.expr_ty.get(lhs.id).copied().unwrap_or(st.tys.error);
        let r = st.expr_ty.get(rhs.id).copied().unwrap_or(st.tys.error);
        if st.tys.is_error(l) || st.tys.is_error(r) {
            return;
        }
        let t = if *op == OperatorKind::Plus && l == st.tys.string && r == st.tys.string {
            st.tys.string
        } else {
            st.tys.join(l, r).unwrap_or(st.tys.error)
        };
        if !st.tys.is_error(t) {
            checked_const_overflow(*op, lhs, rhs, t, e.span, st);
            st.expr_ty.set(e.id, t);
        }
    }
}

fn retype_integer_literal(e: &ExprWrap, target: TyId, st: &mut SemState) {
    let TyKind::Int(kind) = st.tys.kind(target) else {
        return;
    };
    if !matches!(e.kind, ExprKind::Literal(LiteralKind::Integer)) {
        return;
    }
    let cur = st.expr_ty.get(e.id).copied().unwrap_or(st.tys.error);
    if st.tys.is_error(cur) {
        return;
    }
    let Some(v) = parse_integer_literal(e.span.slice(&st.text)) else {
        return;
    };
    let bits = match kind {
        IntKind::I4 => 4u32,
        IntKind::I8 => 8,
        IntKind::I16 => 16,
        IntKind::I32 => 32,
        IntKind::I64 => 64,
        IntKind::I128 => 128,
    };
    if fits_signed(v, bits) {
        st.expr_ty.set(e.id, target);
    } else {
        st.diags.emit(Diagnostic::error(
            DiagCode::IntegerOutOfRange,
            e.span,
            "integer literal out of range for the expected type",
        ));
        st.expr_ty.set(e.id, st.tys.error);
    }
}

/// M1（Q5/D3）：常量整型表达式求值的三态结果
enum ConstInt {
    /// 常量值（在 i128 内精确）
    Val(i128),
    /// 不可常量求值（含非常量操作数）——留给运行期
    NotConst,
    /// 常量除零（D3/D4 编译期诊断）
    ZeroDiv,
    /// 常量 INT_MIN / -1（D4 编译期诊断）
    MinNegOne,
}

/// M1（Q5/D3）：常量整型表达式求值——字面量 / 一元负 / 算术链。
/// 供 checked 的编译期核查；除法零与 MIN/-1 单独成态（诊断码不同）。
fn const_int_expr(e: &ExprWrap, st: &SemState) -> ConstInt {
    match &e.kind {
        ExprKind::Literal(LiteralKind::Integer) => {
            match parse_integer_literal(&st.text[e.span.start as usize..e.span.end as usize]) {
                Some(v) => ConstInt::Val(v),
                None => ConstInt::NotConst,
            }
        }
        ExprKind::Unary {
            op: OperatorKind::Minus,
            operand,
        } => match const_int_expr(operand, st) {
            ConstInt::Val(v) => match v.checked_neg() {
                Some(n) => ConstInt::Val(n),
                None => ConstInt::NotConst,
            },
            other => other,
        },
        ExprKind::Binary { op, lhs, rhs } => {
            match (const_int_expr(lhs, st), const_int_expr(rhs, st)) {
                (ConstInt::Val(l), ConstInt::Val(r)) => eval_arith(*op, l, r),
                // 一侧不可常量或已含除法错误：错误向上传染（首个错误优先）
                (ConstInt::ZeroDiv, _) | (_, ConstInt::ZeroDiv) => ConstInt::ZeroDiv,
                (ConstInt::MinNegOne, _) | (_, ConstInt::MinNegOne) => ConstInt::MinNegOne,
                _ => ConstInt::NotConst,
            }
        }
        _ => ConstInt::NotConst,
    }
}

/// 常量算术求值（两侧已确认为常量值）；/ % 的成态错误在此产出，
/// + - * 的 i128 真溢出返回 NotConst（位宽判据在 checked_const_overflow）
fn eval_arith(op: OperatorKind, l: i128, r: i128) -> ConstInt {
    match op {
        OperatorKind::Plus => match l.checked_add(r) {
            Some(v) => ConstInt::Val(v),
            None => ConstInt::NotConst,
        },
        OperatorKind::Minus => match l.checked_sub(r) {
            Some(v) => ConstInt::Val(v),
            None => ConstInt::NotConst,
        },
        OperatorKind::Star => match l.checked_mul(r) {
            Some(v) => ConstInt::Val(v),
            None => ConstInt::NotConst,
        },
        OperatorKind::Slash => {
            if r == 0 {
                ConstInt::ZeroDiv
            } else if r == -1 && l == i128::MIN {
                ConstInt::MinNegOne
            } else {
                ConstInt::Val(l / r)
            }
        }
        OperatorKind::Percent => {
            if r == 0 {
                ConstInt::ZeroDiv
            } else if r == -1 && l == i128::MIN {
                ConstInt::MinNegOne
            } else {
                ConstInt::Val(l % r)
            }
        }
        _ => ConstInt::NotConst,
    }
}

/// 常量值视图：只有纯值返回 Some（错误态与非常量同视——错误已在更内层钉过）
fn const_value(e: &ExprWrap, st: &SemState) -> Option<i128> {
    match const_int_expr(e, st) {
        ConstInt::Val(v) => Some(v),
        _ => None,
    }
}

/// M1（Q5/D3/D4）：算术两侧皆常量时编译期核查。
///
/// `+ - *` 溢出钉 `SemConstOverflow`；`/ 0` 与 `MIN / -1` 分别钉独立码
/// （D3 裁定：不复用 IntegerOutOfRange——字面量值域与常量算术是两种错误）。
fn checked_const_overflow(
    op: OperatorKind,
    lhs: &ExprWrap,
    rhs: &ExprWrap,
    ty: TyId,
    span: Span,
    st: &mut SemState,
) {
    let w = match st.tys.kind(ty) {
        TyKind::Int(k) => match k {
            IntKind::I4 => 4,
            IntKind::I8 => 8,
            IntKind::I16 => 16,
            IntKind::I32 => 32,
            IntKind::I64 => 64,
            IntKind::I128 => 128,
        },
        _ => return,
    };
    let (Some(l), Some(r)) = (const_value(lhs, st), const_value(rhs, st)) else {
        return;
    };
    match op {
        OperatorKind::Plus | OperatorKind::Minus | OperatorKind::Star => {
            let v = match op {
                OperatorKind::Plus => l.checked_add(r),
                OperatorKind::Minus => l.checked_sub(r),
                _ => l.checked_mul(r),
            };
            let overflow = match v {
                Some(v) => !fits_signed(v, w),
                None => true,
            };
            if overflow {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemConstOverflow,
                    span,
                    "constant arithmetic overflows the result type (wrapping: use +% -% *%)",
                ));
            }
        }
        OperatorKind::Slash | OperatorKind::Percent => {
            let diag = if r == 0 {
                Some((
                    DiagCode::SemDivisionByZero,
                    "division by zero in constant expression",
                ))
            } else if r == -1 && l == i128::MIN {
                Some((
                    DiagCode::SemDivisionOverflow,
                    "constant division overflows (MIN / -1)",
                ))
            } else {
                None
            };
            if let Some((code, msg)) = diag {
                st.diags.emit(Diagnostic::error(code, span, msg));
            }
        }
        _ => {}
    }
}
fn parse_integer_literal(text: &str) -> Option<i128> {
    let cleaned: String = text.replace('_', "");
    let (digits, radix) = match cleaned.as_str() {
        s if s.len() > 2 && s[..2].eq_ignore_ascii_case("0x") => (&s[2..], 16),
        s if s.len() > 2 && s[..2].eq_ignore_ascii_case("0o") => (&s[2..], 8),
        s if s.len() > 2 && s[..2].eq_ignore_ascii_case("0b") => (&s[2..], 2),
        s => (s, 10),
    };
    i128::from_str_radix(digits, radix).ok()
}

fn fits_signed(v: i128, bits: u32) -> bool {
    if bits >= 128 {
        return true;
    }
    let max = (1i128 << (bits - 1)) - 1;
    let min = -(1i128 << (bits - 1));
    (min..=max).contains(&v)
}

fn type_expr_inner(e: &ExprWrap, st: &mut SemState, in_value: bool) -> TyId {
    match &e.kind {
        ExprKind::Error(_) | ExprKind::Missing(_) => st.tys.error,
        ExprKind::Literal(lk) => match lk {
            LiteralKind::Integer => integer_literal_ty(e, st),
            LiteralKind::Float => st.tys.default_float,
            LiteralKind::String => st.tys.string,
            LiteralKind::Char => st.tys.char_,
        },
        ExprKind::Bool(_) => st.tys.bool_,
        ExprKind::Null => st.tys.null,
        ExprKind::Ident(_) | ExprKind::Path(_) => match st.expr_def.get(e.id).copied() {
            Some(def) => {
                // M4 slice 4b (Plan C)：weak 绑定读取 = upgrade——定型
                // 为 ?P；irc 在读取点发射 starry_weak_upgrade（+1 临时值
                // 纪律：绑定/返回转移，语句末 release）
                if matches!(
                    st.defs.get(def).kind,
                    DefKind::LocalBinding { weak: true, .. }
                ) {
                    let vty = def_value_ty(def, st);
                    return st.tys.nullable(vty);
                }
                def_value_ty(def, st)
            }
            None => st.tys.error,
        },
        // M2 取址 &x / &mut x（T1 借用；v1 保守规则见 semantic-notes）
        ExprKind::AddrOf { mutable, expr } => {
            let inner = type_expr(expr, st, true);
            check_borrow(expr, *mutable, st);
            st.tys.intern(TyKind::Ref {
                mutable: *mutable,
                inner,
            })
        }
        // M2 解引 *p：pointer/ref → 内层值
        ExprKind::Deref { expr } => {
            let pty = type_expr(expr, st, true);
            match st.tys.kind(pty).clone() {
                TyKind::Pointer { inner, .. } => {
                    require_unsafe_if_raw_deref(st, pty, e.span);
                    inner
                }
                TyKind::Ref { inner, .. } => inner,
                TyKind::Error => st.tys.error,
                _ => {
                    mismatch(st, expr.span);
                    st.tys.error
                }
            }
        }
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
            let from = type_expr(expr, st, true);
            let target = st.ty_of_node.get(ty.id).copied().unwrap_or(st.tys.error);
            // P0 compiler trust：as 只允许数值↔数值、同型、Ref/Pointer 互转
            //（D-007/M2：指针创建/转换 safe，解引用才 gated）——其余在 sem
            // 稳定拒收，杜绝「check clean、LLVM 才炸」。
            let allowed = st.tys.is_error(from)
                || st.tys.is_error(target)
                || from == target
                || (st.tys.is_numeric(from) && st.tys.is_numeric(target))
                || matches!(
                    (st.tys.kind(from), st.tys.kind(target)),
                    (TyKind::Ref { .. }, TyKind::Pointer { .. })
                        | (TyKind::Pointer { .. }, TyKind::Ref { .. })
                        | (TyKind::Pointer { .. }, TyKind::Pointer { .. })
                );
            if !allowed {
                mismatch(st, e.span);
                return st.tys.error;
            }
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
            // M5 slice 2b：命名空间限定访问——base 解析为 PackageNs
            //（`import util` 后 `util.Helper` / `a.b.C`）按成员名解析
            // 并写 expr_def，类型走 def_value_ty（class 构造器等）
            if !*safe
                && let Some(bdef) = st.expr_def.get(base.id).copied()
                && matches!(st.defs.get(bdef).kind, DefKind::PackageNs)
            {
                let use_scope = st.node_scope.get(e.id).copied().unwrap_or(st.scopes.root());
                return match member_lookup(st, bdef, *name) {
                    Some(mdef) if !crate::declare::vis_allows(st, mdef, use_scope) => {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            e.span,
                            format!(
                                "`{}` is not visible here (internal to another package)",
                                st.interner.resolve(st.defs.get(mdef).name)
                            ),
                        ));
                        st.tys.error
                    }
                    Some(mdef) => {
                        st.expr_def.set(e.id, mdef);
                        def_value_ty(mdef, st)
                    }
                    None => {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnknownMember,
                            e.span,
                            format!("unknown member `{}`", st.interner.resolve(*name)),
                        ));
                        st.tys.error
                    }
                };
            }
            // M4 slice 4b (Plan C)：经 weak 的 `?.` 安全访问放行
            //（base 读取即 upgrade，定型 ?P）；非安全 `.` 仍拒（需显式 !!）
            if !*safe
                && let ExprKind::Ident(_) = &base.kind
                && let Some(def) = st.expr_def.get(base.id).copied()
                && matches!(
                    st.defs.get(def).kind,
                    DefKind::LocalBinding { weak: true, .. }
                )
            {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnsupportedConstruct,
                    e.span,
                    "accessing a member through a `weak` binding requires upgrade: use `?.` or `!!`",
                ));
                return st.tys.error;
            }
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
        ExprKind::Loop { body, .. } => {
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
                // 全裸 break（零带值）= unit 结果，不报错；混用（部分带值）才是类型错误
                if frame.break_with_value != 0 && frame.break_with_value != frame.break_total {
                    mismatch(st, e.span);
                }
                let mut result = frame.break_values.first().copied().unwrap_or(st.tys.unit);
                // skip(1)：裸 `loop { break }`（值列表空）曾在此 [1..] 越界 panic
                for ty in frame.break_values.iter().skip(1) {
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
        ExprKind::Comptime { block } => {
            // P0 compiler trust：同 StmtKind::Comptime——稳定拒收 + 定型防级联
            unsupported(st, e.span, "comptime blocks are not supported in v1");
            type_block(block, st, in_value);
            st.tys.error
        }
        // M6：unsafe 块建立 extern "c" 调用的许可上下文（language-design：
        // FFI 调用在 unsafe 中）。深度计数覆盖嵌套块。
        ExprKind::Unsafe { block } => {
            st.unsafe_depth += 1;
            let ty = type_block(block, st, in_value);
            st.unsafe_depth -= 1;
            ty
        }
        ExprKind::Lambda { params, body } => {
            // M8 slice 2: pop expected Fn type（check_call_args 实参或
            // S2/P2-3b 绑定注解压栈）
            let expected = st.lambda_exp.pop().flatten();
            // create param scope with bindings
            let parent = st
                .node_scope
                .get(e.id)
                .copied()
                .unwrap_or_else(|| st.scopes.root());
            let scope = st.scopes.new_scope(Some(parent));
            // S2/P2-3b：推断回退——无期望 Fn 类型时，显式参数全带类型
            // 标注则从标注构造参数表，返回型由 body 推断
            //（`val f = { (n: int) -> n + 1 }`）；`it` 形式与无标注参数
            // 仍稳定拒收（无信息可推）
            let (param_tys, ret_ty, infer_ret) = match expected {
                Some(ft) => match st.tys.kind(ft).clone() {
                    TyKind::Fn(ps, r) => (ps, r, false),
                    _ => {
                        unsupported(st, e.span, "expected type is not a function type");
                        return st.tys.error;
                    }
                },
                None => match lambda_params_from_annotations(params, scope, st) {
                    Some(ps) => (ps, st.tys.unit, true),
                    None => {
                        unsupported(
                            st,
                            e.span,
                            "cannot infer lambda param types without function type context",
                        );
                        return st.tys.error;
                    }
                },
            };
            let mut sig_params: Vec<(Symbol, TyId)> = Vec::new();
            match params {
                LambdaParams::It => {
                    let sym = st.interner.intern("it");
                    let ty = param_tys.first().copied().unwrap_or(st.tys.error);
                    let def = st.defs.push(crate::def::Def {
                        kind: DefKind::Param,
                        name: sym,
                        scope,
                        span: e.span,
                        sig: {
                            let mut s = crate::def::DefSig::new(st.tys.unit, st.tys.error);
                            s.ty = ty;
                            s
                        },
                        resolving: false,
                        alias_of: None,
                        alias_resolved: None,
                        member_scope: None,
                        external: false,
                        ty_params: 0,
                        decl: None,
                    });
                    st.scopes.insert(scope, sym, def);
                    sig_params.push((sym, ty));
                }
                LambdaParams::Explicit(_) => {
                    // S2/P2-1 spike 修复：`{ (a, b) -> … }` 整体是单元素元组
                    // 模式（= 参数表）——经 param_kinds 展平后逐个绑定
                    for (i, kind) in params.param_kinds().iter().enumerate() {
                        let ty = param_tys.get(i).copied().unwrap_or(st.tys.error);
                        if let PatKind::Binding { name, .. } = kind {
                            let def = st.defs.push(crate::def::Def {
                                kind: DefKind::Param,
                                name: *name,
                                scope,
                                span: e.span,
                                sig: {
                                    let mut s = crate::def::DefSig::new(st.tys.unit, st.tys.error);
                                    s.ty = ty;
                                    s
                                },
                                resolving: false,
                                alias_of: None,
                                alias_resolved: None,
                                member_scope: None,
                                external: false,
                                ty_params: 0,
                                decl: None,
                            });
                            st.scopes.insert(scope, *name, def);
                            sig_params.push((*name, ty));
                        }
                    }
                }
            }
            // create synthetic fun def
            let lambda_def = st.defs.push(crate::def::Def {
                kind: DefKind::Fun,
                name: st.interner.intern("__lambda"),
                scope,
                span: e.span,
                sig: {
                    let mut s = crate::def::DefSig::new(st.tys.unit, st.tys.error);
                    s.params = sig_params.clone();
                    s.ret = ret_ty;
                    s
                },
                resolving: false,
                alias_of: None,
                alias_resolved: None,
                member_scope: None,
                external: false,
                ty_params: 0,
                decl: None,
            });
            st.lambda_defs.insert(e.id.0, lambda_def);
            // type body in lambda scope
            st.funs.push(crate::FunFrame {
                node: e.id,
                def: lambda_def,
            });
            // M8: resolve lambda body identifiers in lambda scope
            // S2/P2-3：捕获窗口覆盖 resolve + type（赋值拒收在 type 期）
            st.lambda_cap_stack.push((e.id.0, scope, Vec::new()));
            // A closure is a new function boundary; lexical unsafe permission
            // does not leak from its creation site into its body.
            let outer_unsafe_depth = st.unsafe_depth;
            st.unsafe_depth = 0;
            crate::resolve::resolve_block(body, st, scope);
            let body_ty = type_block(body, st, true);
            let (_, _, caps) = st.lambda_cap_stack.pop().expect("capture stack");
            st.unsafe_depth = outer_unsafe_depth;
            st.lambda_caps.insert(e.id.0, caps);
            st.funs.pop();
            let ret_ty = if infer_ret {
                body_ty
            } else {
                if !assignable_with_unsize(st, body_ty, ret_ty) && !st.tys.is_error(body_ty) {
                    mismatch(st, e.span);
                }
                ret_ty
            };
            // S2/P2-3b：推断路径的 def sig.ret 是压栈占位（unit）——
            // 回填 body 推断型（irc lambda_lower 按 def sig lower 签名）
            if infer_ret {
                st.defs.get_mut(lambda_def).sig.ret = body_ty;
            }
            st.tys.intern(TyKind::Fn(param_tys, ret_ty))
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
/// S2/P2-3b：无期望 Fn 类型时的 lambda 参数推断——显式参数全部携带
/// 类型标注（`{ (n: int) -> … }`）才可行；任一参数缺标注或 `it` 形式
/// 返回 None（调用方稳定拒收）。零参 lambda（`{ () -> e }`）返回空表。
fn lambda_params_from_annotations(
    params: &LambdaParams,
    scope: crate::def::ScopeId,
    st: &mut SemState,
) -> Option<Vec<TyId>> {
    match params {
        LambdaParams::It => None,
        LambdaParams::Explicit(_) => {
            let mut out = Vec::new();
            for kind in params.param_kinds() {
                match kind {
                    PatKind::Binding {
                        ty: Some(ty_wrap), ..
                    } => {
                        out.push(crate::declare::lower_ty(st, scope, ty_wrap));
                    }
                    _ => return None,
                }
            }
            Some(out)
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
                    Some(t) if st.tys.is_numeric(t) => {
                        // M1（Q5）：常量表达式在目标位宽上可证明的溢出，
                        // 编译期诊断而非留给运行期 trap
                        checked_const_overflow(op, lhs, rhs, t, span, st);
                        t
                    }
                    _ => {
                        mismatch(st, span);
                        st.tys.error
                    }
                }
            }
        }
        Op::Minus | Op::Star => match st.tys.join(l, r) {
            Some(t) if st.tys.is_numeric(t) => {
                checked_const_overflow(op, lhs, rhs, t, span, st);
                t
            }
            _ => {
                mismatch(st, span);
                st.tys.error
            }
        },
        Op::Slash | Op::Percent => match st.tys.join(l, r) {
            Some(t) if st.tys.is_numeric(t) => {
                checked_const_overflow(op, lhs, rhs, t, span, st);
                t
            }
            _ => {
                mismatch(st, span);
                st.tys.error
            }
        },
        // M1（Q5）：wrapping 算术——类型规则同受检算子（`+%` 不做字符串连接）
        Op::PlusPercent | Op::MinusPercent | Op::StarPercent => match st.tys.join(l, r) {
            // wrapping 仅整型：浮点无回绕语义（避免落入 interp 的 bitop trap）
            Some(t) if st.tys.is_integral(t) => t,
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
            // M4 slice 2a：class 引用相等 = 指针比较（R4 裁定默认引用相等）。
            // class 值已标量化（is_aggregate=false），走标量 Bin Eq——
            // llvme 发 icmp eq ptr；不得落入聚合结构相等（会把 header 算进去）
            if crate::declare::is_class_ty(st, l) || crate::declare::is_class_ty(st, r) {
                if !(crate::declare::is_class_ty(st, l) && crate::declare::is_class_ty(st, r)) {
                    mismatch(st, span);
                    return st.tys.error;
                }
                return st.tys.bool_;
            }
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
            if assignable_with_unsize(st, r, inner) {
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
/// M7 slice 2：调用点类型实参推断（type_args 省略时）。
/// 实参**恰好定型一次**（coverage 纪律）：命名/位置入槽、dup/缺省核查、
/// 与含 Param 标记的形参统一、未绑定兜底默认值，最后按代入后形参做
/// assignable 核查。诊断已发时返回 None。
fn infer_call_targs(
    st: &mut SemState,
    want: usize,
    params: &[(Symbol, TyId)],
    param_defaults: &[bool],
    tp_defaults: &[Option<TyId>],
    args: &[CallArg],
    span: Span,
) -> Option<Vec<TyId>> {
    let mut bindings: Vec<Option<TyId>> = vec![None; want];
    // (实参类型, 实参 span) 按形参槽位记录
    let mut slots: Vec<Option<(TyId, Span)>> = vec![None; params.len()];
    let mut bad = false;
    for (i, a) in args.iter().enumerate() {
        let aty = type_expr(&a.value, st, true);
        let pidx = match a.name {
            Some(n) => params.iter().position(|(pn, _)| *pn == n),
            None => Some(i),
        };
        let Some(pi) = pidx else {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnknownParam,
                a.value.span,
                "named argument has no matching parameter",
            ));
            bad = true;
            continue;
        };
        if pi >= params.len() {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemArityMismatch,
                span,
                "too many arguments",
            ));
            bad = true;
            continue;
        }
        if slots[pi].is_some() {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemArityMismatch,
                span,
                "duplicate argument for one parameter",
            ));
            bad = true;
            continue;
        }
        slots[pi] = Some((aty, a.value.span));
        unify_param(st, params[pi].1, aty, &mut bindings);
    }
    for (j, slot) in slots.iter().enumerate() {
        if slot.is_none() && !param_defaults.get(j).copied().unwrap_or(false) {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemArityMismatch,
                span,
                format!(
                    "missing argument for `{}`",
                    st.interner.resolve(params[j].0)
                ),
            ));
            bad = true;
        }
    }
    for (i, b) in bindings.iter_mut().enumerate() {
        if b.is_none()
            && let Some(d) = tp_defaults.get(i).copied().flatten()
        {
            *b = Some(d);
        }
    }
    if bindings.iter().any(|b| b.is_none()) {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemTypeArgMismatch,
            span,
            "cannot infer all type arguments; specify them explicitly or add type parameter defaults",
        ));
        bad = true;
    }
    if bad {
        return None;
    }
    let targs: Vec<TyId> = bindings
        .into_iter()
        .map(|b| b.expect("checked above"))
        .collect();
    // 代入后 assignable 核查（实参已定型，不重定型）
    for (j, slot) in slots.iter().enumerate() {
        if let Some((aty, aspan)) = slot {
            let sub = st.tys.subst(params[j].1, &targs);
            if !assignable_with_unsize(st, *aty, sub) {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemTypeMismatch,
                    *aspan,
                    "argument type does not match the inferred instantiation",
                ));
            }
        }
    }
    Some(targs)
}

/// M7 slice 2：形参类型（含 TyKind::Param 标记）与实参类型的结构统一。
/// 冲突保留首个绑定（兜底由代入后 assignable 核查报告）。
fn unify_param(st: &SemState, pty: TyId, aty: TyId, bindings: &mut [Option<TyId>]) {
    match (st.tys.kind(pty).clone(), st.tys.kind(aty).clone()) {
        (TyKind::Param(i), _) => {
            let i = i as usize;
            if i < bindings.len() && bindings[i].is_none() {
                bindings[i] = Some(aty);
            }
        }
        (TyKind::Named(pd, pa), TyKind::Named(ad, aa)) if pd == ad && pa.len() == aa.len() => {
            for (x, y) in pa.iter().zip(aa.iter()) {
                unify_param(st, *x, *y, bindings);
            }
        }
        (TyKind::Nullable(p), TyKind::Nullable(a)) => unify_param(st, p, a, bindings),
        (TyKind::Array(p, _), TyKind::Array(a, _)) => unify_param(st, p, a, bindings),
        (TyKind::Tuple(ps), TyKind::Tuple(as_)) if ps.len() == as_.len() => {
            for (x, y) in ps.iter().zip(as_.iter()) {
                unify_param(st, *x, *y, bindings);
            }
        }
        (TyKind::Ref { inner: p, .. }, TyKind::Ref { inner: a, .. }) => {
            unify_param(st, p, a, bindings);
        }
        (TyKind::Pointer { inner: p, .. }, TyKind::Pointer { inner: a, .. }) => {
            unify_param(st, p, a, bindings);
        }
        (TyKind::Range(p), TyKind::Range(a)) => unify_param(st, p, a, bindings),
        _ => {}
    }
}
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
    // M5 slice 2b：命名空间限定调用（`import util` 后 `util.Helper(...)`）——
    // base 解析为 PackageNs 时按成员解析并写 expr_def，类型走 def_value_ty
    //（class 构造器 = TypeObject，落到下游统一 ctor 核查）
    if let ExprKind::Member { base, name, safe } = &callee.kind
        && !*safe
        && let Some(bdef) = st.expr_def.get(base.id).copied()
        && matches!(st.defs.get(bdef).kind, DefKind::PackageNs)
    {
        // coverage 纪律：base 表达式必须定型（即便其值类型是 error）
        let _base_ty = type_expr(base, st, true);
        let use_scope = st
            .node_scope
            .get(callee.id)
            .copied()
            .unwrap_or(st.scopes.root());
        return match member_lookup(st, bdef, *name) {
            Some(mdef) if !crate::declare::vis_allows(st, mdef, use_scope) => {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnsupportedConstruct,
                    callee.span,
                    format!(
                        "`{}` is not visible here (internal to another package)",
                        st.interner.resolve(st.defs.get(mdef).name)
                    ),
                ));
                type_all_args(args, st);
                st.tys.error
            }
            Some(mdef) => {
                st.expr_def.set(callee.id, mdef);
                let ty = def_value_ty(mdef, st);
                if st.tys.is_error(ty) {
                    type_all_args(args, st);
                    return st.tys.error;
                }
                st.expr_ty.set(callee.id, ty);
                match st.tys.kind(ty).clone() {
                    TyKind::TypeObject(named) => {
                        // 与下游 TypeObject 臂同纪律：构造器实参核查
                        if !matches!(st.defs.get(named).kind, DefKind::Struct | DefKind::Class) {
                            type_all_args(args, st);
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemTypeMismatch,
                                callee.span,
                                "this type name is not callable",
                            ));
                            return st.tys.error;
                        }
                        let (params, defaults) = {
                            let sig = &st.defs.get(named).sig;
                            (sig.params.clone(), sig.param_defaults.clone())
                        };
                        check_call_args(args, &params, &defaults, span, st);
                        st.tys.builtin_named(named)
                    }
                    TyKind::Fn(_, ret) => {
                        // 形参名经 def 签名取（TyKind::Fn 只有类型序）
                        let (params, defaults) = {
                            let sig = &st.defs.get(mdef).sig;
                            (sig.params.clone(), sig.param_defaults.clone())
                        };
                        check_call_args(args, &params, &defaults, span, st);
                        ret
                    }
                    _ => {
                        type_all_args(args, st);
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemTypeMismatch,
                            callee.span,
                            "namespace member is not callable",
                        ));
                        st.tys.error
                    }
                }
            }
            None => {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnknownMember,
                    callee.span,
                    format!("unknown member `{}`", st.interner.resolve(*name)),
                ));
                type_all_args(args, st);
                st.tys.error
            }
        };
    }
    // Member 目标的定型与消歧一次完成：字段（含函数字段）/枚举变体查得到，
    // 查不到即方法调用（由此处报「不支持」，不让 member_field_ty 发 unknown-field 误导）
    let callee_ty = match &callee.kind {
        ExprKind::Member { base, name, safe } => {
            // M4 slice 4b (Plan C)：经 weak 的 `?.` 方法调用放行；非安全拒
            if !*safe
                && let ExprKind::Ident(_) = &base.kind
                && let Some(def) = st.expr_def.get(base.id).copied()
                && matches!(
                    st.defs.get(def).kind,
                    DefKind::LocalBinding { weak: true, .. }
                )
            {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnsupportedConstruct,
                    callee.span,
                    "calling a method through a `weak` binding requires upgrade: use `?.` or `!!`",
                ));
                return st.tys.error;
            }
            let base_ty = type_expr(base, st, true);
            let fty = if st.tys.is_error(base_ty) {
                st.tys.error
            } else {
                match member_field_ty_ex(base_ty, *name, *safe, callee.span, st, false) {
                    Some(t) => t,
                    None => {
                        // M4 slice 2b：class 方法分派（静态单态；self 在 lowering 注入）
                        let method = if let TyKind::Named(def, _) = st.tys.kind(base_ty).clone()
                            && matches!(st.defs.get(def).kind, DefKind::Class)
                            && let Some(m) = member_lookup(st, def, *name)
                            && matches!(st.defs.get(m).kind, DefKind::Fun)
                        {
                            Some(m)
                        } else {
                            None
                        };
                        // Phase 2a: dynamic dispatch resolves to the trait's
                        // declared method slot; IRC loads that vtable index.
                        let dyn_trait = match st.tys.kind(base_ty) {
                            TyKind::Dyn(trait_def, _) => Some(*trait_def),
                            TyKind::Nullable(inner) => match st.tys.kind(*inner) {
                                TyKind::Dyn(trait_def, _) => Some(*trait_def),
                                _ => None,
                            },
                            _ => None,
                        };
                        let dyn_method = if let Some(tdef) = dyn_trait {
                            member_lookup(st, tdef, *name)
                                .filter(|mdef| matches!(st.defs.get(*mdef).kind, DefKind::Fun))
                        } else {
                            None
                        };
                        let method = if dyn_method.is_some() {
                            dyn_method
                        } else {
                            method
                        };
                        match method {
                            Some(m) => {
                                let params = st.defs.get(m).sig.params.clone();
                                let defaults = st.defs.get(m).sig.param_defaults.clone();
                                check_call_args(args, &params, &defaults, span, st);
                                let raw_ret = st.defs.get(m).sig.ret;
                                let ret = if *safe
                                    && matches!(st.tys.kind(base_ty), TyKind::Nullable(inner) if matches!(st.tys.kind(*inner), TyKind::Dyn(..)))
                                {
                                    st.tys.nullable(raw_ret)
                                } else {
                                    raw_ret
                                };
                                st.expr_def.set(callee.id, m);
                                // callee 值占位（irc 方法路径读 expr_def；早退绕过
                                // 臂尾的 expr_ty.set，这里必须自写）
                                st.expr_ty.set(callee.id, st.tys.unit);
                                return ret;
                            }
                            None => {
                                // M9 9b：trait impl 静态分派——按
                                // (基类型 def, 方法名) 查注册表；唯一命中
                                // 即分派目标（self 由 irc 注入）；多命中 = 歧义
                                let tdef = match st.tys.kind(base_ty) {
                                    TyKind::Named(d, _) => Some(*d),
                                    _ => None,
                                };
                                let candidates: Vec<DefId> = match tdef {
                                    Some(d) => st
                                        .impls
                                        .iter()
                                        .filter(|im| {
                                            // M9 9d: exact match OR same-name generic impl
                                            im.type_def == d
                                                || (st.defs.get(im.type_def).name
                                                    == st.defs.get(d).name
                                                    && st.defs.get(im.type_def).ty_params > 0)
                                        })
                                        .filter_map(|im| {
                                            im.methods
                                                .iter()
                                                .find(|(n, _)| *n == *name)
                                                .map(|(_, md)| *md)
                                        })
                                        .collect(),
                                    None => Vec::new(),
                                };
                                match candidates.as_slice() {
                                    [m] => {
                                        let params = st.defs.get(*m).sig.params.clone();
                                        let defaults = st.defs.get(*m).sig.param_defaults.clone();
                                        check_call_args(args, &params, &defaults, span, st);
                                        let ret = st.defs.get(*m).sig.ret;
                                        st.expr_def.set(callee.id, *m);
                                        st.expr_ty.set(callee.id, st.tys.unit);
                                        return ret;
                                    }
                                    [] => {
                                        unsupported(
                                            st,
                                            callee.span,
                                            "method calls are not supported in semantic v1",
                                        );
                                        st.tys.error
                                    }
                                    _ => {
                                        st.diags.emit(Diagnostic::error(
                                            DiagCode::SemUnsupportedConstruct,
                                            callee.span,
                                            format!(
                                                "ambiguous method `{}`: multiple trait impls provide it",
                                                st.interner.resolve(*name)
                                            ),
                                        ));
                                        type_all_args(args, st);
                                        st.tys.error
                                    }
                                }
                            }
                        }
                    }
                }
            };
            st.expr_ty.set(callee.id, fty);
            fty
        }
        _ => type_expr(callee, st, true),
    };
    // M7：类型实参在 resolve 期已 lower（ty_of_node 缓存）；之后的
    // 分派按被调是否泛型逐个臂处理，毯式拒收已拆除
    let targs: Vec<TyId> = type_args
        .iter()
        .map(|ta| st.ty_of_node.get(ta.ty.id).copied().unwrap_or(st.tys.error))
        .collect();
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
                if !type_args.is_empty() {
                    // M7 slice 1：泛型 enum 未排期，变体调用也不带类型实参
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemTypeArgMismatch,
                        span,
                        "enum variant calls do not take type arguments",
                    ));
                }
                let resolved =
                    member_lookup(st, named, *name).and_then(|v| match st.defs.get(v).kind {
                        DefKind::EnumVariant { parent, index } => Some((parent, index)),
                        _ => None,
                    });
                if let Some((parent, index)) = resolved {
                    let vparams = variant_arg_tys(parent, index, st);
                    // M7 slice 2：泛型 enum——变体实参推断 + 实例化 spec，
                    // 变体 def 重映射到 spec 的对应变体（irc variant_ctor
                    // 读 spec 的代入后签名）
                    if st.defs.get(named).ty_params > 0 {
                        let tp_defaults = st.defs.get(named).sig.tp_defaults.clone();
                        let Some(targs) = infer_call_targs(
                            st,
                            st.defs.get(named).ty_params as usize,
                            &vparams,
                            &vec![false; vparams.len()],
                            &tp_defaults,
                            args,
                            span,
                        ) else {
                            return st.tys.error;
                        };
                        let spec = crate::mono::instantiate_enum(st, named, &targs);
                        let Some(svariant) = member_lookup(st, spec, *name) else {
                            return st.tys.error;
                        };
                        let (sparent, sindex) = match st.defs.get(svariant).kind {
                            DefKind::EnumVariant { parent, index } => (parent, index),
                            _ => return st.tys.error,
                        };
                        let svparams = variant_arg_tys(sparent, sindex, st);
                        for (a, (_, pt)) in args.iter().zip(svparams.iter()) {
                            let aty = st.expr_ty.get(a.value.id).copied().unwrap_or(st.tys.error);
                            if !assignable_with_unsize(st, aty, *pt) {
                                st.diags.emit(Diagnostic::error(
                                    DiagCode::SemTypeMismatch,
                                    a.value.span,
                                    "argument type does not match the inferred instantiation",
                                ));
                            }
                        }
                        st.expr_def.set(callee.id, svariant);
                        return st.tys.builtin_named(spec);
                    }
                    check_call_args(args, &vparams, &[], span, st);
                } else {
                    type_all_args(args, st);
                }
                return st.tys.builtin_named(named);
            }
        }
        TyKind::TypeObject(named) => match st.defs.get(named).kind.clone() {
            // M4 slice 1：class 构造——槽机核查同 struct；expr_def 指向
            // class def（irc 构造者判定免 retain 的依据）
            DefKind::Class => {
                // 主构造：sig.params（含仅构造参数）优先，无头构造退 fields
                let sig = &st.defs.get(named).sig;
                if !sig.params.is_empty() {
                    let params = sig.params.clone();
                    let defaults = sig.param_defaults.clone();
                    check_call_args(args, &params, &defaults, span, st);
                } else {
                    let fields = sig.fields.clone();
                    check_call_args(args, &fields, &[], span, st);
                }
                st.expr_def.set(callee.id, named);
                return st.tys.builtin_named(named);
            }
            DefKind::Struct => {
                // M7：泛型 struct 构造——实例化 spec def，实参按代入后 sig 核查
                if st.defs.get(named).ty_params > 0 {
                    let want = st.defs.get(named).ty_params as usize;
                    // M7 slice 2：type_args 省略时从构造实参推断
                    let targs = if type_args.is_empty() {
                        let sig = &st.defs.get(named).sig;
                        let (params, pdefaults, tpdefs) = if !sig.params.is_empty() {
                            (
                                sig.params.clone(),
                                sig.param_defaults.clone(),
                                sig.tp_defaults.clone(),
                            )
                        } else {
                            (
                                sig.fields.clone(),
                                vec![false; sig.fields.len()],
                                sig.tp_defaults.clone(),
                            )
                        };
                        let Some(t) =
                            infer_call_targs(st, want, &params, &pdefaults, &tpdefs, args, span)
                        else {
                            return st.tys.error;
                        };
                        t
                    } else {
                        if targs.len() != want {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemTypeArgMismatch,
                                span,
                                format!(
                                    "generic struct expects {want} type argument(s), got {}",
                                    targs.len()
                                ),
                            ));
                            type_all_args(args, st);
                            return st.tys.error;
                        }
                        targs
                    };
                    let spec = crate::mono::instantiate(st, named, &targs);
                    // irc 的 call 分派按 expr_def 找构造目标（克隆 spec）
                    st.expr_def.set(callee.id, spec);
                    if type_args.is_empty() {
                        // M7 slice 2：推断路径实参已定型核查；不重定型
                        return st.tys.builtin_named(spec);
                    }
                    let sig = &st.defs.get(spec).sig;
                    if !sig.params.is_empty() {
                        // M3 主构造：参数槽机（含仅构造参数与默认值）
                        let params = sig.params.clone();
                        let defaults = sig.param_defaults.clone();
                        check_call_args(args, &params, &defaults, span, st);
                    } else {
                        let fields = sig.fields.clone();
                        check_call_args(args, &fields, &[], span, st);
                    }
                    return st.tys.builtin_named(spec);
                }
                if !type_args.is_empty() {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemTypeArgMismatch,
                        span,
                        "this struct is not generic",
                    ));
                }
                let sig = &st.defs.get(named).sig;
                if !sig.params.is_empty() {
                    // M3 主构造：参数槽机（含仅构造参数与默认值）
                    let params = sig.params.clone();
                    let defaults = sig.param_defaults.clone();
                    check_call_args(args, &params, &defaults, span, st);
                } else {
                    let fields = sig.fields.clone();
                    check_call_args(args, &fields, &[], span, st);
                }
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
                    let sig = &st.defs.get(t).sig;
                    if !sig.params.is_empty() {
                        let params = sig.params.clone();
                        let defaults = sig.param_defaults.clone();
                        check_call_args(args, &params, &defaults, span, st);
                    } else {
                        let fields = sig.fields.clone();
                        check_call_args(args, &fields, &[], span, st);
                    }
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
        // M6：extern "c" 调用必须在 unsafe 上下文中（FFI 永远 unchecked）
        if st.defs.get(def).external && st.unsafe_depth == 0 {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                callee.span,
                "calling an `extern \"c\"` function requires an `unsafe` block",
            ));
        }
        // M7：泛型 fun 调用——取/建实例化，expr_def 指向 spec def
        // （irc 按它 lower 克隆体），实参按代入后形参核查
        if st.defs.get(def).ty_params > 0 {
            let want = st.defs.get(def).ty_params as usize;
            // M7 slice 2：type_args 省略时从实参类型推断（失败已诊断）
            let targs = if type_args.is_empty() {
                let sig = &st.defs.get(def).sig;
                let (params, pdefaults, tpdefs) = (
                    sig.params.clone(),
                    sig.param_defaults.clone(),
                    sig.tp_defaults.clone(),
                );
                let Some(t) = infer_call_targs(st, want, &params, &pdefaults, &tpdefs, args, span)
                else {
                    return st.tys.error;
                };
                t
            } else {
                if targs.len() != want {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemTypeArgMismatch,
                        span,
                        format!(
                            "generic function expects {want} type argument(s), got {}",
                            targs.len()
                        ),
                    ));
                    type_all_args(args, st);
                    return st.tys.error;
                }
                targs
            };
            let spec = crate::mono::instantiate(st, def, &targs);
            st.expr_def.set(callee.id, spec);
            let (params, ret, defaults) = crate::mono::subst_params(st, def, &targs);
            // M7 slice 2：推断路径的实参已定型并核查（coverage 纪律），
            // 仅显式实参路径走 check_call_args
            if type_args.is_empty() {
                return ret;
            }
            check_call_args(args, &params, &defaults, span, st);
            return ret;
        }
        if !type_args.is_empty() {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemTypeArgMismatch,
                span,
                "this function is not generic",
            ));
        }
        let params = st.defs.get(def).sig.params.clone();
        let defaults = st.defs.get(def).sig.param_defaults.clone();
        let ret = st.defs.get(def).sig.ret;
        check_call_args(args, &params, &defaults, span, st);
        return ret;
    }
    // 函数值调用（`(int) -> int` 型的形参/绑定作 callee）
    if let TyKind::Fn(ps, ret) = st.tys.kind(callee_ty).clone() {
        if !type_args.is_empty() {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemTypeArgMismatch,
                span,
                "function values do not take type arguments",
            ));
        }
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
            if !assignable_with_unsize(st, at, *p) {
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
        // M8 slice 2：lambda 实参压入期望类型供 Lambda 弹出
        if matches!(&arg.value.kind, ExprKind::Lambda { .. }) {
            st.lambda_exp.push(Some(params[idx].1));
        }
        let aty = type_expr(&arg.value, st, true);
        // M1：形参是具体整型且实参是整数字面量 → 按形参宽度重定型
        retype_binary_literals(&arg.value, params[idx].1, st);
        retype_integer_literal(&arg.value, params[idx].1, st);
        slots[idx] = Some(st.expr_ty.get(arg.value.id).copied().unwrap_or(aty));
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
        if !assignable_with_unsize(st, slot.unwrap_or(st.tys.error), params[i].1) {
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
                    if !st.tys.is_error(sty)
                        && t != st.tys.bool_
                        && !assignable_with_unsize(st, t, sty)
                    {
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
                // M7 slice 2：泛型 enum 实例化 spec 的穷尽性/覆盖核查按
                // spec 自身变体（covered_variants 已在 refine 时重映射）
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
            // M7 slice 2：泛型 enum 模式的 ctor 重映射——subject 是
            // 实例化 spec（Named(spec, [])）时把 pat_res.ctor 从泛型
            // 原件的变体重映射到 spec 的对应变体（类型代入后）
            #[allow(clippy::collapsible_if)]
            if let Some(spec) = (match st.tys.kind(subject_ty).clone() {
                TyKind::Named(spec, _)
                    if matches!(st.defs.get(spec).kind, DefKind::Enum)
                        && st.defs.get(spec).ty_params == 0 =>
                {
                    Some(spec)
                }
                _ => None,
            }) && matches!(st.defs.get(spec).kind, DefKind::Enum)
                && st.defs.get(spec).ty_params == 0
                && let Some(existing_ctor) = st.pat_res.get(pat.id).copied()
                && let DefKind::EnumVariant { parent, .. } =
                    st.defs.get(existing_ctor.ctor).kind.clone()
                && st.defs.get(parent).ty_params > 0
                && st
                    .mono_insts
                    .iter()
                    .any(|(o, s)| o.0 == parent && *s == spec)
            {
                if let Some(svariant) =
                    member_lookup(st, spec, st.defs.get(existing_ctor.ctor).name)
                    && let DefKind::EnumVariant { index, .. } = st.defs.get(svariant).kind.clone()
                {
                    st.pat_res.set(
                        pat.id,
                        PatRes {
                            ctor: svariant,
                            variant: Some(index),
                        },
                    );
                }
            }
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
                // M7 slice 1：泛型 struct 模式未排期（字段类型是 Param，无可精化）
                if st.defs.get(res.ctor).ty_params > 0 {
                    unsupported(
                        st,
                        pat.span,
                        "generic struct patterns are not supported in this slice",
                    );
                    return;
                }
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
        // M5 slice 2a：包命名空间在值/类型位置使用 = 误用（import 路径经 expand_import 处理）
        DefKind::PackageNs => st.tys.error,
        // M9 9a：trait 名在值位置 = 误用（静态分派 only；dyn 后置 M11）
        DefKind::Trait => st.tys.error,
        // 别名在值位置归一到目标：TypeObject(alias_resolved)，
        // 成员访问/模式路径/构造经目标 DefId 统一解析（type_call 的
        // TypeAlias 臂仅兜未 lower 的别名）
        DefKind::TypeAlias => {
            let target = st.defs.get(def).alias_resolved.unwrap_or(def);
            st.tys.intern(TyKind::TypeObject(target))
        }
        // M7：类型形参名出现在值位置 = 类型/值分立违规
        DefKind::TypeParam { .. } => st.tys.error,
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
        // 已有错误诊断时跳过覆盖断言：出错子树本就不定型，断言只会
        // 以 panic 盖过真实诊断（debug 可用性）
        if st
            .diags
            .all()
            .iter()
            .any(|d| matches!(d.severity, ast::syntax::node::DiagSeverity::Error))
        {
            return;
        }
        for (i, item) in ast.items.iter().enumerate() {
            match &item.kind {
                DeclKind::Fun(f) => {
                    // M7：泛型原件不检查（无 expr_ty 属预期）；克隆照常断言
                    // （区分靠 def：spec def 的 ty_params = 0）
                    let is_generic_orig = st
                        .decl_def
                        .get(i)
                        .copied()
                        .flatten()
                        .is_some_and(|d| st.defs.get(d).ty_params > 0);
                    if is_generic_orig {
                        continue;
                    }
                    match &f.body {
                        Some(FunctionBody::Block(b)) => block(b, st),
                        Some(FunctionBody::Expr(e)) => expr(e, st),
                        None => {}
                    }
                }
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
        if st.expr_ty.get(id).is_none() {
            eprintln!("UNTYPED {id:?}");
        }
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
