//! Pass 2：resolve —— 名字 → DefId；label/flow 目标；pattern 构造器解析
//!
//! 只绑定、不定型。`is` 单段构造器模式依赖主语类型，留待 check 精化
//! （pat_res 在 check 补写）；lambda/spawn 内部整体跳过（见 semantic-notes）。

use ast::interner::Symbol;
use ast::syntax::decl::{DeclKind, FunDecl, FunctionBody};
use ast::syntax::expr::*;
use ast::syntax::node::{DiagCode, Diagnostic, NodeId, SourceFile, Span};
use ast::syntax::pat::{PatKind, PatWrap};
use ast::syntax::stmt::{StmtKind, StmtWrap};

use crate::declare::{lookup_with_imports, lower_ty, resolve_path};
use crate::def::{Def, DefKind, DefSig, ScopeId};
use crate::ty::TyId;
use crate::{FlowKind, FlowRes, FunFrame, LabelFrame, PatRes, SemState};

pub(crate) fn run(ast: &SourceFile, st: &mut SemState) {
    let root = st.scopes.root();
    for item in &ast.items {
        match &item.kind {
            DeclKind::Fun(f) => resolve_fun(item.id, f, st),
            DeclKind::Binding(b) => {
                if let Some(init) = &b.init {
                    resolve_expr(init, st, root);
                }
            }
            _ => {}
        }
    }
}

fn zero_span() -> Span {
    Span { start: 0, end: 0 }
}

fn resolve_fun(decl_id: NodeId, f: &FunDecl, st: &mut SemState) {
    let root = st.scopes.root();
    let Some(fdef) = st.scopes.lookup_local(root, f.name) else {
        return;
    };
    // 参数作用域：参数 defs 的 ty 已在 declare 期 lower 进 fun sig
    let sig_params = st.defs.get(fdef).sig.params.clone();
    let pscope = st.scopes.new_scope(Some(root));
    for (name, ty) in sig_params {
        let def = st.defs.push(Def {
            kind: DefKind::Param,
            name,
            scope: pscope,
            span: zero_span(),
            sig: {
                let mut s = DefSig::new(st.tys.unit, st.tys.unknown);
                s.ty = ty;
                s
            },
            resolving: false,
            alias_of: None,
            alias_resolved: None,
            member_scope: None,
        });
        if let Some(old) = st.scopes.insert(pscope, name, def) {
            // 保留首个参数定义，重复项报错（与 add_def 同纪律）
            st.scopes.insert(pscope, name, old);
            st.diags.emit(Diagnostic::error(
                DiagCode::SemDuplicateDef,
                zero_span(),
                format!("duplicate parameter `{}`", st.interner.resolve(name)),
            ));
        }
    }
    st.funs.push(FunFrame {
        node: decl_id,
        def: fdef,
    });
    match &f.body {
        Some(FunctionBody::Block(b)) => resolve_block(b, st, pscope),
        Some(FunctionBody::Expr(e)) => resolve_expr(e, st, pscope),
        None => {}
    }
    st.funs.pop();
}

fn resolve_block(b: &Block, st: &mut SemState, parent: ScopeId) {
    let scope = st.scopes.new_scope(Some(parent));
    for s in &b.stmts {
        resolve_stmt(s, st, scope);
    }
    if let Some(t) = &b.tail {
        resolve_expr(t, st, scope);
    }
}

fn resolve_stmt(s: &StmtWrap, st: &mut SemState, scope: ScopeId) {
    match &s.kind {
        StmtKind::Expr(es) => resolve_expr(&es.expr, st, scope),
        StmtKind::Assignment { target, value, .. } => {
            resolve_expr(target, st, scope);
            resolve_expr(value, st, scope);
        }
        StmtKind::Binding(b) => {
            if let Some(init) = &b.init {
                resolve_expr(init, st, scope);
            }
            declare_local_binding(&b.pattern, b.immutable, scope, st);
            if let Some(t) = &b.ty {
                let ty = lower_ty(st, scope, t);
                note_binding_ann(st, &b.pattern, ty);
            }
        }
        StmtKind::For {
            label,
            pattern,
            iterable,
            body,
        } => {
            resolve_expr(iterable, st, scope);
            let lscope = st.scopes.new_scope(Some(scope));
            declare_local_binding(pattern, true, lscope, st);
            // 无标签帧也压栈：无标签 break/continue 取栈顶即最近循环
            st.labels.push(LabelFrame {
                label: *label,
                node: s.id,
            });
            resolve_block(body, st, lscope);
            st.labels.pop();
        }
        StmtKind::While { label, cond, body } => {
            // 条件绑定作用域必须是循环体的祖先（`while val x = ... { x }`）
            let cscope = resolve_condition(cond, st, scope);
            let lscope = st.scopes.new_scope(Some(cscope));
            st.labels.push(LabelFrame {
                label: *label,
                node: s.id,
            });
            resolve_block(body, st, lscope);
            st.labels.pop();
        }
        StmtKind::Loop { label, body } => {
            let lscope = st.scopes.new_scope(Some(scope));
            st.labels.push(LabelFrame {
                label: *label,
                node: s.id,
            });
            resolve_block(body, st, lscope);
            st.labels.pop();
        }
        StmtKind::Break { label, value } => {
            if let Some(v) = value {
                resolve_expr(v, st, scope);
            }
            resolve_jump_target(st, s.id, s.span, *label, FlowKind::Break);
        }
        StmtKind::Continue { label } => {
            resolve_jump_target(st, s.id, s.span, *label, FlowKind::Continue);
        }
        StmtKind::Return { value } => {
            if let Some(v) = value {
                resolve_expr(v, st, scope);
            }
            match st.funs.last() {
                Some(f) => {
                    st.flow_res.set(
                        s.id,
                        FlowRes {
                            target: f.node,
                            kind: FlowKind::Return,
                        },
                    );
                }
                None => {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemReturnOutsideFun,
                        s.span,
                        "return outside any function",
                    ));
                }
            }
        }
        StmtKind::Defer { body } => resolve_block(body, st, scope),
        StmtKind::Comptime { block } => resolve_block(block, st, scope),
        StmtKind::Grow { expr, bits } => {
            resolve_expr(expr, st, scope);
            if let Some(b) = bits {
                resolve_expr(b, st, scope);
            }
        }
        StmtKind::Error(_) | StmtKind::Missing(_) => {}
    }
}

fn resolve_jump_target(
    st: &mut SemState,
    stmt: NodeId,
    span: Span,
    label: Option<Symbol>,
    kind: FlowKind,
) {
    let target = match label {
        Some(l) => st
            .labels
            .iter()
            .rev()
            .find(|f| f.label == Some(l))
            .map(|f| f.node),
        None => st.labels.last().map(|f| f.node),
    };
    match (target, label) {
        (Some(node), _) => {
            st.flow_res.set(stmt, FlowRes { target: node, kind });
        }
        (None, Some(_)) => {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnknownLabel,
                span,
                "label has no matching loop",
            ));
        }
        (None, None) => {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemBreakOutsideLoop,
                span,
                "break/continue outside any loop",
            ));
        }
    }
}

/// 条件解析；返回绑定生效的作用域（Expr 条件返回原作用域，
/// Val 条件返回新开的模式绑定作用域——调用方须以它为体块父作用域）
fn resolve_condition(cond: &Condition, st: &mut SemState, scope: ScopeId) -> ScopeId {
    match cond {
        Condition::Expr(e) => {
            resolve_expr(e, st, scope);
            scope
        }
        Condition::Val { pattern, expr } => {
            resolve_expr(expr, st, scope);
            let pscope = st.scopes.new_scope(Some(scope));
            declare_local_binding(pattern, true, pscope, st);
            pscope
        }
    }
}

pub(crate) fn resolve_expr(e: &ExprWrap, st: &mut SemState, scope: ScopeId) {
    match &e.kind {
        ExprKind::Ident(name) => {
            st.node_scope.set(e.id, scope);
            match lookup_with_imports(st, scope, *name) {
                Some(def) => {
                    st.expr_def.set(e.id, def);
                }
                None => {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemUndefinedName,
                        e.span,
                        format!("undefined name `{}`", st.interner.resolve(*name)),
                    ));
                }
            }
        }
        ExprKind::Path(p) => {
            st.node_scope.set(e.id, scope);
            match resolve_path(st, scope, p) {
                Some(def) => {
                    st.expr_def.set(e.id, def);
                }
                None => {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemUndefinedName,
                        e.span,
                        "undefined path",
                    ));
                }
            }
        }
        ExprKind::Binary { lhs, rhs, .. } => {
            resolve_expr(lhs, st, scope);
            resolve_expr(rhs, st, scope);
        }
        ExprKind::Is { lhs, pattern, .. } => {
            resolve_expr(lhs, st, scope);
            declare_local_binding(pattern, true, scope, st);
            resolve_pattern(pattern, st, scope);
        }
        // in 右操作数是集合表达式（EBNF RangeExpr 侧）：不产生模式绑定
        ExprKind::In { lhs, rhs, .. } => {
            resolve_expr(lhs, st, scope);
            resolve_expr(rhs, st, scope);
        }
        ExprKind::Unary { operand, .. } => resolve_expr(operand, st, scope),
        ExprKind::ForceUnwrap { expr: operand } => resolve_expr(operand, st, scope),
        ExprKind::Cast { expr, ty, .. } => {
            resolve_expr(expr, st, scope);
            lower_ty(st, scope, ty);
        }
        ExprKind::Call {
            callee,
            type_args,
            args,
        } => {
            for ta in type_args {
                lower_ty(st, scope, &ta.ty);
            }
            // Member callee 的名字解析推迟到 check（依赖 base 类型）
            match &callee.kind {
                ExprKind::Member { base, .. } => resolve_expr(base, st, scope),
                _ => resolve_expr(callee, st, scope),
            }
            for arg in args {
                resolve_expr(&arg.value, st, scope);
            }
        }
        ExprKind::Index { base, index } => {
            resolve_expr(base, st, scope);
            resolve_expr(index, st, scope);
        }
        ExprKind::Member { base, .. } => resolve_expr(base, st, scope),
        ExprKind::Tuple(xs) | ExprKind::Array(xs) => {
            for x in xs {
                resolve_expr(x, st, scope);
            }
        }
        ExprKind::ArrayRepeat { value, .. } => resolve_expr(value, st, scope),
        ExprKind::StringInterp { parts } => {
            for p in parts {
                resolve_expr(p, st, scope);
            }
        }
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => {
            if let Condition::Val { pattern, expr } = &**cond {
                resolve_expr(expr, st, scope);
                let pscope = st.scopes.new_scope(Some(scope));
                declare_local_binding(pattern, true, pscope, st);
                resolve_block(then_block, st, pscope);
                if let Some(eb) = else_block {
                    resolve_block(eb, st, scope);
                }
                return;
            }
            match &**cond {
                Condition::Expr(c) => resolve_expr(c, st, scope),
                Condition::Val { .. } => unreachable!(),
            }
            resolve_block(then_block, st, scope);
            if let Some(eb) = else_block {
                resolve_block(eb, st, scope);
            }
        }
        ExprKind::When { subject, branches } => {
            if let Some(s) = subject {
                resolve_expr(s, st, scope);
            }
            for b in branches {
                let bscope = st.scopes.new_scope(Some(scope));
                match &b.pattern {
                    WhenPattern::Expression(e) => resolve_expr(e, st, bscope),
                    WhenPattern::Pattern(p) => {
                        // 构造器参数绑定（如 Opt.Some(x) 的 x）注册进分支作用域
                        declare_local_binding(p, true, bscope, st);
                        resolve_pattern(p, st, bscope);
                    }
                    WhenPattern::Else => {}
                }
                if let Some(g) = &b.guard {
                    resolve_expr(g, st, bscope);
                }
                match &b.body {
                    WhenBody::Expr(e) => resolve_expr(e, st, bscope),
                    WhenBody::Block(b) => resolve_block(b, st, bscope),
                }
            }
        }
        // 跳过的区域（semantic-notes 有记录）：lambda/spawn 内部不解析
        ExprKind::Lambda { .. } | ExprKind::Spawn(_) => {}
        ExprKind::Async { block } => {
            resolve_block(block, st, scope);
        }
        ExprKind::Await { expr } => {
            resolve_expr(expr, st, scope);
        }
        ExprKind::Concurrent { block } => {
            resolve_block(block, st, scope);
        }
        ExprKind::Comptime { block } | ExprKind::Unsafe { block } => {
            resolve_block(block, st, scope);
        }
        ExprKind::Proceed | ExprKind::Super { .. } | ExprKind::MacroCall { .. } => {}
        ExprKind::Literal(_) | ExprKind::Bool(_) | ExprKind::Null => {}
        ExprKind::Error(_) | ExprKind::Missing(_) => {}
    }
}

/// 局部绑定引入（val/var、for/if-let/when 分支模式内的 Binding）
pub(crate) fn declare_local_binding(
    pat: &PatWrap,
    immutable: bool,
    scope: ScopeId,
    st: &mut SemState,
) {
    fn walk(pat: &PatWrap, immutable: bool, scope: ScopeId, st: &mut SemState) {
        match &pat.kind {
            PatKind::Binding { name, .. } => {
                let def = st.defs.push(Def {
                    kind: DefKind::LocalBinding {
                        mutable: !immutable,
                    },
                    name: *name,
                    scope,
                    span: pat.span,
                    sig: DefSig::new(st.tys.unit, st.tys.unknown),
                    resolving: false,
                    alias_of: None,
                    alias_resolved: None,
                    member_scope: None,
                });
                if st.scopes.insert(scope, *name, def).is_some() {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemDuplicateDef,
                        pat.span,
                        format!("duplicate binding `{}`", st.interner.resolve(*name)),
                    ));
                }
                st.pat_binding_def.set(pat.id, def);
            }
            // 或模式各替代绑定同名同变量：只注册首个替代，
            // 否则 `A(x) | B(x) -> x` 在同作用域被误报重名
            PatKind::Or(ps) => {
                if let Some(first) = ps.first() {
                    walk(first, immutable, scope, st);
                }
            }
            PatKind::Tuple { elements: ps, rest } | PatKind::Array { elements: ps, rest } => {
                if rest.is_some() {
                    // rest 解构的 lowering 未排期（irc 拒收）：sem 先行诊断
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemUnsupportedConstruct,
                        pat.span,
                        "rest patterns (`..`) are not supported in semantic v1",
                    ));
                }
                for p in ps {
                    walk(p, immutable, scope, st);
                }
            }
            PatKind::Constructor { args, .. } => {
                for p in args {
                    walk(p, immutable, scope, st);
                }
            }
            PatKind::Struct { fields, .. } => {
                for (_, p) in fields {
                    walk(p, immutable, scope, st);
                }
            }
            _ => {}
        }
    }
    walk(pat, immutable, scope, st);
}

fn note_binding_ann(st: &mut SemState, pat: &PatWrap, ty: TyId) {
    if let Some(&def) = st.pat_binding_def.get(pat.id) {
        st.defs.get_mut(def).sig.ty = ty;
    }
    match &pat.kind {
        PatKind::Or(ps)
        | PatKind::Tuple { elements: ps, .. }
        | PatKind::Array { elements: ps, .. } => {
            for p in ps {
                note_binding_ann(st, p, ty);
            }
        }
        _ => {}
    }
}

/// 模式解析：构造器（2 段路径直接定 variant；1 段留 check 精化）、struct 字段核查
pub(crate) fn resolve_pattern(pat: &PatWrap, st: &mut SemState, scope: ScopeId) {
    match &pat.kind {
        PatKind::Binding { ty: Some(t), .. } => {
            let lowered = lower_ty(st, scope, t);
            note_binding_ann(st, pat, lowered);
        }
        PatKind::Or(ps)
        | PatKind::Tuple { elements: ps, .. }
        | PatKind::Array { elements: ps, .. } => {
            for p in ps {
                resolve_pattern(p, st, scope);
            }
        }
        PatKind::Constructor { path, args } => {
            match resolve_path(st, scope, path) {
                Some(def) => {
                    let ok = match &st.defs.get(def).kind {
                        DefKind::EnumVariant { .. } => true,
                        DefKind::Enum if path.segments.len() == 1 => true, // check 精化
                        _ => false,
                    };
                    if ok {
                        let variant = match &st.defs.get(def).kind {
                            DefKind::EnumVariant { index, .. } => Some(*index),
                            _ => None,
                        };
                        st.pat_res.set(pat.id, PatRes { ctor: def, variant });
                    } else {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnknownMember,
                            pat.span,
                            "constructor pattern does not name an enum variant",
                        ));
                    }
                }
                None => {
                    if path.segments.len() != 1 {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUndefinedName,
                            pat.span,
                            "undefined constructor path",
                        ));
                    }
                    // 单段：可能是主语枚举的变体名，check 依主语精化时再判定
                }
            }
            for a in args {
                resolve_pattern(a, st, scope);
            }
        }
        PatKind::Struct { path, fields } => match resolve_path(st, scope, path) {
            Some(def) if matches!(st.defs.get(def).kind, DefKind::Struct) => {
                st.pat_res.set(
                    pat.id,
                    PatRes {
                        ctor: def,
                        variant: None,
                    },
                );
                let field_names: Vec<Symbol> = st
                    .defs
                    .get(def)
                    .sig
                    .fields
                    .iter()
                    .map(|(n, _)| *n)
                    .collect();
                for (name, fp) in fields {
                    if !field_names.contains(name) {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnknownMember,
                            fp.span,
                            format!("unknown struct field `{}`", st.interner.resolve(*name)),
                        ));
                    }
                    resolve_pattern(fp, st, scope);
                }
            }
            _ => {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUndefinedType,
                    pat.span,
                    "struct pattern does not name a struct",
                ));
            }
        },
        PatKind::Range { lo, hi, .. } => {
            resolve_pattern(lo, st, scope);
            resolve_pattern(hi, st, scope);
        }
        _ => {}
    }
}
