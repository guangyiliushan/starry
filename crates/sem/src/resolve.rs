//! Pass 2：resolve —— 名字 → DefId；label/flow 目标；pattern 构造器解析
//!
//! 只绑定、不定型。`is` 单段构造器模式依赖主语类型，留待 check 精化
//! （pat_res 在 check 补写）；lambda/spawn 内部整体跳过（见 semantic-notes）。

use ast::interner::Symbol;
use ast::syntax::decl::{ClassMember, DeclKind, FunDecl, FunctionBody, TraitMember};
use ast::syntax::expr::*;
use ast::syntax::node::{DiagCode, Diagnostic, NodeId, SourceFile, Span};
use ast::syntax::pat::{PatKind, PatWrap};
use ast::syntax::stmt::{StmtKind, StmtWrap};

use crate::declare::{lookup_with_imports, lower_ty, member_lookup, resolve_path};
use crate::def::{Def, DefId, DefKind, DefSig, ScopeId};
use crate::ty::TyId;
use crate::{FlowKind, FlowRes, FunFrame, LabelFrame, PatRes, SemState};

pub(crate) fn run(ast: &SourceFile, st: &mut SemState) {
    run_from(ast, st, 0);
}

/// M7 增量入口：只处理克隆追加的 items[start..]（原件跑 run = run_from(.., 0)）
pub(crate) fn run_from(ast: &SourceFile, st: &mut SemState, start: usize) {
    let root = st.scopes.root();
    for (i, item) in ast.items.iter().enumerate().skip(start) {
        match &item.kind {
            DeclKind::Fun(f) => {
                // M7：def 经 decl_def 侧链取（克隆 def 与原件同名同 span，
                // 名字/span 反查都会撞车）
                let Some(fdef) = st.decl_def.get(i).copied().flatten() else {
                    continue;
                };
                resolve_fun(item.id, f, fdef, st)
            }
            // M4 slice 2b：class 成员函数与 deinit 的解析（self 注入作用域）
            DeclKind::Class(c) => {
                let Some(class_def) = st.decl_def.get(i).copied().flatten() else {
                    continue;
                };
                let self_ty = st.tys.builtin_named(class_def);
                let self_name = st.interner.intern("self");
                // D-019: base initializer expressions are checked in the
                // derived primary-constructor parameter scope. Keep this scope
                // on node_scope so check uses exactly the same lexical view.
                let ctor_scope = st.scopes.new_scope(Some(st.defs.get(class_def).scope));
                if let Some(pc) = &c.primary_ctor {
                    let sig_params = st.defs.get(class_def).sig.params.clone();
                    for p in &pc.params {
                        let ty = sig_params
                            .iter()
                            .find(|(n, _)| *n == p.name)
                            .map(|(_, ty)| *ty)
                            .unwrap_or(st.tys.unknown);
                        let binding = st.defs.push(Def {
                            kind: DefKind::LocalBinding {
                                mutable: false,
                                weak: false,
                            },
                            name: p.name,
                            scope: ctor_scope,
                            span: item.span,
                            sig: DefSig::new(st.tys.unit, st.tys.unknown),
                            resolving: false,
                            alias_of: None,
                            alias_resolved: None,
                            member_scope: None,
                            external: false,
                            ty_params: 0,
                            decl: None,
                        });
                        st.defs.get_mut(binding).sig.ty = ty;
                        st.scopes.insert(ctor_scope, p.name, binding);
                    }
                }
                st.node_scope.set(item.id, ctor_scope);
                if let Some(super_item) = c.supertypes.first()
                    && let Some(args) = &super_item.args
                {
                    for arg in args {
                        resolve_expr(&arg.value, st, ctor_scope);
                    }
                }
                for member in &c.members {
                    match member {
                        ClassMember::Declaration(d) if matches!(&d.kind, DeclKind::Fun(_)) => {
                            if let DeclKind::Fun(f) = &d.kind {
                                let Some(mdef) = member_lookup(st, class_def, f.name) else {
                                    continue;
                                };
                                resolve_method(d.id, f, mdef, self_name, self_ty, st);
                            }
                        }
                        ClassMember::Deinit(b) => {
                            let dname = st.interner.intern("deinit");
                            let Some(mdef) = member_lookup(st, class_def, dname) else {
                                continue;
                            };
                            resolve_deinit(b, mdef, self_name, self_ty, st);
                        }
                        _ => {}
                    }
                }
            }
            // M9 9b：impl 方法体解析——self = impl 目标类型
            //（静态分派挂钩；与 class 成员同纪律）
            DeclKind::Impl(i) => {
                let Some(entry) = st.impls.iter().find(|im| im.span == item.span).cloned() else {
                    continue;
                };
                let self_name = st.interner.intern("self");
                let self_ty = st.tys.builtin_named(entry.type_def);
                for member in &i.members {
                    if let ClassMember::Declaration(d) = member
                        && let DeclKind::Fun(f) = &d.kind
                        && let Some((_, mdef)) = entry.methods.iter().find(|(n, _)| *n == f.name)
                    {
                        resolve_method(d.id, f, *mdef, self_name, self_ty, st);
                    }
                }
            }
            // WP5: trait default bodies are checked once against the trait's
            // Self placeholder. Impl dispatch reuses the lowered method def.
            DeclKind::Trait(t) => {
                let Some(tdef) = st.decl_def.get(i).copied().flatten() else {
                    continue;
                };
                let Some(ms) = st.defs.get(tdef).member_scope else {
                    continue;
                };
                let self_name = st.interner.intern("self");
                let self_key = st.interner.intern("Self");
                let Some(self_def) = st.scopes.lookup_local(ms, self_key) else {
                    continue;
                };
                let self_ty = st.tys.builtin_named(self_def);
                for member in &t.members {
                    if let TraitMember::Fun(f) = member
                        && f.body.is_some()
                        && let Some(mdef) = member_lookup(st, tdef, f.name)
                    {
                        resolve_method(item.id, f, mdef, self_name, self_ty, st);
                    }
                }
            }
            DeclKind::Struct(s) => {
                // M3/M5 2a：主构造默认值在所属包作用域解析
                let dscope = item_scope(i, st).unwrap_or(root);
                for p in &s.primary_ctor {
                    if let Some(d) = &p.default {
                        resolve_expr(d, st, dscope);
                    }
                }
            }
            DeclKind::Binding(b) => {
                if let Some(init) = &b.init {
                    resolve_expr(init, st, item_scope(i, st).unwrap_or(root));
                }
            }
            _ => {}
        }
    }
}

/// M5 slice 2a：item 的声明作用域（= 所属包作用域；无 def 返回 None）
fn item_scope(i: usize, st: &SemState) -> Option<ScopeId> {
    let d = st.decl_def.get(i).copied().flatten()?;
    Some(st.defs.get(d).scope)
}

fn zero_span() -> Span {
    Span { start: 0, end: 0 }
}

/// M4 slice 2b：建 self 参数 def 的作用域（成员函数与 deinit 共用）
fn self_scope(class_def: DefId, self_name: Symbol, self_ty: TyId, st: &mut SemState) -> ScopeId {
    // M5 slice 2a：self 作用域父级 = class 所属包作用域（包名/import 可见）
    let parent = st.defs.get(class_def).scope;
    let sscope = st.scopes.new_scope(Some(parent));
    let unit = st.tys.unit;
    let self_def = st.defs.push(Def {
        kind: DefKind::Param,
        name: self_name,
        scope: sscope,
        span: zero_span(),
        sig: {
            let mut s = DefSig::new(unit, st.tys.unknown);
            s.ty = self_ty;
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
    st.scopes.insert(sscope, self_name, self_def);
    // D-019: inherited fields are visible through `self` in derived methods.
    // Primary-constructor parameters must not leak into method bodies.
    let mut cursor = class_def;
    loop {
        let fields = st.defs.get(cursor).sig.fields.clone();
        for (index, (name, ty)) in fields.iter().enumerate() {
            let field_def = st.defs.push(Def {
                kind: DefKind::StructField {
                    parent: cursor,
                    index,
                    mutable: None,
                },
                name: *name,
                scope: sscope,
                span: zero_span(),
                sig: {
                    let mut sig = DefSig::new(unit, st.tys.unknown);
                    sig.ty = *ty;
                    sig
                },
                resolving: false,
                alias_of: None,
                alias_resolved: None,
                member_scope: None,
                external: false,
                ty_params: 0,
                decl: None,
            });
            st.scopes.insert(sscope, *name, field_def);
        }
        let Some(base) = st
            .defs
            .get(cursor)
            .sig
            .supertypes
            .first()
            .copied()
            .filter(|base| matches!(st.defs.get(*base).kind, DefKind::Class))
        else {
            break;
        };
        cursor = base;
    }
    sscope
}

fn resolve_method(
    decl_id: NodeId,
    f: &FunDecl,
    mdef: DefId,
    self_name: Symbol,
    self_ty: TyId,
    st: &mut SemState,
) {
    let sscope = self_scope(mdef, self_name, self_ty, st);
    // sig.ty 复用为 self 的 class 值类型（irc 方法 lowering/调用注入的通道）
    st.defs.get_mut(mdef).sig.ty = self_ty;
    resolve_fun_with_parent(decl_id, f, mdef, sscope, st);
}

fn resolve_deinit(block: &Block, mdef: DefId, self_name: Symbol, self_ty: TyId, st: &mut SemState) {
    let sscope = self_scope(mdef, self_name, self_ty, st);
    st.defs.get_mut(mdef).sig.ty = self_ty;
    // deinit 块无独立 NodeId 头——用 block 首语句 id 作帧锚（仅 flow 目标用）
    let anchor_id = block.stmts.first().map(|s| s.id).unwrap_or(NodeId(0));
    st.funs.push(FunFrame {
        node: anchor_id,
        def: mdef,
    });
    resolve_block(block, st, sscope);
    st.funs.pop();
}

fn resolve_fun(decl_id: NodeId, f: &FunDecl, fdef: DefId, st: &mut SemState) {
    // M7：泛型 fun 的 pscope 挂在类型形参作用域下（member_scope 复用）；
    // M5 slice 2a：非泛型回退 = fun 所属包作用域（内建在根，链可达）
    let parent = st
        .defs
        .get(fdef)
        .member_scope
        .unwrap_or_else(|| st.defs.get(fdef).scope);
    resolve_fun_with_parent(decl_id, f, fdef, parent, st);
}

/// M4 slice 2b：class 成员函数/deinit 的解析——parent 是含 self 的作用域
fn resolve_fun_with_parent(
    decl_id: NodeId,
    f: &FunDecl,
    fdef: DefId,
    parent: ScopeId,
    st: &mut SemState,
) {
    let root = st.defs.get(fdef).scope; // M5 2a：默认实参在所属包作用域解析
    // 参数作用域：参数 defs 的 ty 已在 declare 期 lower 进 fun sig。
    let sig_params = st.defs.get(fdef).sig.params.clone();
    let pscope = st.scopes.new_scope(Some(parent));
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
            external: false,
            ty_params: 0,
            decl: None,
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
    // 默认实参表达式在根作用域解析（只允许引用全局/函数；形参引用
    // 需要调用点绑定语义，v1 未实现，按未定义名拒绝）。
    for p in &f.params {
        if let Some(default) = &p.default {
            resolve_expr(default, st, root);
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

pub(crate) fn resolve_block(b: &Block, st: &mut SemState, parent: ScopeId) {
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
            let weak = b
                .modifiers
                .modifiers
                .iter()
                .any(|m| m.word == ast::token::ModifierWord::Weak);
            if weak && b.immutable {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnsupportedConstruct,
                    b.pattern.span,
                    "`weak` requires a `var` binding",
                ));
            }
            declare_local_binding(&b.pattern, b.immutable, weak, scope, st);
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
            declare_local_binding(pattern, true, false, lscope, st);
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
            declare_local_binding(pattern, true, false, pscope, st);
            // 与 ExprKind::Is 同纪律：构造器路径 resolve 期解析（pat_res），
            // 否则两段构造器（Opt.Some(w)）在 if val 里不做 payload 精化
            resolve_pattern(pattern, st, pscope);
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
                    // R4/M4 契约：裸字段名不可作标识符（irc 仅绑定 self）；
                    // 成员体内访问字段必须 `self.field`——统一在此守卫
                    if matches!(st.defs.get(def).kind, DefKind::StructField { .. }) {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            e.span,
                            format!(
                                "field `{}` requires `self.` inside member functions",
                                st.interner.resolve(*name)
                            ),
                        ));
                    }
                    // S2/P2-3：lambda 捕获分析——lambda 作用域子树外的
                    // 局部/形参 = 捕获（按值：标量快照 / class 引用）；
                    // mutable 全局稳定拒收；val 全局直读不捕获
                    if !st.lambda_cap_stack.is_empty() {
                        let dscope = st.defs.get(def).scope;
                        let dty = st.defs.get(def).sig.ty;
                        let dmut = matches!(
                            st.defs.get(def).kind,
                            DefKind::LocalBinding { mutable: true, .. }
                        );
                        let is_binding = matches!(
                            st.defs.get(def).kind,
                            DefKind::LocalBinding { .. } | DefKind::Param
                        );
                        let lscope = st.lambda_cap_stack.last().unwrap().1;
                        let mut s = Some(dscope);
                        let mut inside = false;
                        while let Some(x) = s {
                            if x == lscope {
                                inside = true;
                                break;
                            }
                            s = st.scopes.parent_of(x);
                        }
                        if is_binding && !inside {
                            let is_global = st
                                .funs
                                .first()
                                .map(|f| st.defs.get(f.def).scope == dscope)
                                .unwrap_or(false);
                            if is_global && dmut {
                                let text = st.interner.resolve(*name);
                                st.diags.emit(Diagnostic::error(
                                    DiagCode::SemUnsupportedConstruct,
                                    e.span,
                                    format!("lambda cannot reference mutable global `{text}` (S2 value capture)"),
                                ));
                            } else if !is_global
                                && let Some((_, _, caps)) = st.lambda_cap_stack.last_mut()
                                && !caps.iter().any(|(s, _)| *s == *name)
                            {
                                caps.push((*name, dty));
                            }
                        }
                    }
                    st.expr_def.set(e.id, def);
                }
                None => {
                    // `self`/`Self` 是 EBNF 关键字（parser 落成同名 Ident）：
                    // 成员函数之外给指向性诊断，而非「未定义名」
                    let text = st.interner.resolve(*name);
                    if text == "self" || text == "Self" {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemSelfOutsideMember,
                            e.span,
                            "`self`/`Self` are only available inside class member functions (not supported in semantic v1)",
                        ));
                    } else {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUndefinedName,
                            e.span,
                            format!("undefined name `{}`", text),
                        ));
                    }
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
            declare_local_binding(pattern, true, false, scope, st);
            resolve_pattern(pattern, st, scope);
        }
        // in 右操作数是集合表达式（EBNF RangeExpr 侧）：不产生模式绑定
        ExprKind::In { lhs, rhs, .. } => {
            resolve_expr(lhs, st, scope);
            resolve_expr(rhs, st, scope);
        }
        ExprKind::Unary { operand, .. } => resolve_expr(operand, st, scope),
        ExprKind::AddrOf { expr, .. } | ExprKind::Deref { expr } => resolve_expr(expr, st, scope),
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
        ExprKind::Index { base, index, .. } => {
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
                declare_local_binding(pattern, true, false, pscope, st);
                // 与 is/while val 同纪律：两段构造器 resolve 期解析（pat_res）
                resolve_pattern(pattern, st, pscope);
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
                        declare_local_binding(p, true, false, bscope, st);
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
        ExprKind::Lambda { .. } => {
            st.node_scope.set(e.id, scope);
        }
        ExprKind::Spawn(_) => {}
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
        ExprKind::Loop { label, body } => {
            // loop 表达式同样提供 break/continue 目标（无标签帧 = 最近循环）
            st.labels.push(LabelFrame {
                label: *label,
                node: e.id,
            });
            resolve_block(body, st, scope);
            st.labels.pop();
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
    weak: bool,
    scope: ScopeId,
    st: &mut SemState,
) {
    fn walk(pat: &PatWrap, immutable: bool, weak: bool, scope: ScopeId, st: &mut SemState) {
        match &pat.kind {
            PatKind::Binding { name, .. } => {
                let def = st.defs.push(Def {
                    kind: DefKind::LocalBinding {
                        weak,
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
                    external: false,
                    ty_params: 0,
                    decl: None,
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
                    walk(first, immutable, weak, scope, st);
                }
            }
            PatKind::Tuple { elements: ps, .. } | PatKind::Array { elements: ps, .. } => {
                for p in ps {
                    walk(p, immutable, weak, scope, st);
                }
            }
            PatKind::Constructor { args, .. } => {
                for p in args {
                    walk(p, immutable, weak, scope, st);
                }
            }
            PatKind::Struct { fields, .. } => {
                for (_, p) in fields {
                    walk(p, immutable, weak, scope, st);
                }
            }
            _ => {}
        }
    }
    walk(pat, immutable, weak, scope, st);
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
