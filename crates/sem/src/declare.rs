//! Pass 1：declare —— 作用域树 + defs + 顶层签名 eager lower
//!
//! 前向引用是硬约束（Starry 允许文件内先用后声明），故名字全量注册
//! 先于一切解析；签名类型在名字注册完成后统一 lower（别名循环守卫
//! `Def.resolving`）。

use ast::interner::Symbol;
use ast::syntax::decl::*;
use ast::syntax::node::{DiagCode, Diagnostic, SourceFile, Span};
use ast::syntax::ty::{TypeKind, TypeWrap};

use crate::SemState;
use crate::def::{Def, DefId, DefKind, DefSig, ScopeId, Vis};
use crate::ty::{FloatKind, IntKind, TyArena, TyId, TyKind};

const BUILTINS: &[&str] = &[
    "i4", "i8", "i16", "i32", "i64", "i128", "int", "f32", "f64", "bool", "string", "char", "unit",
    "Own", "Shared",
];

pub(crate) fn run(ast: &SourceFile, st: &mut SemState) {
    register_builtins(st);
    let root = st.scopes.root();
    // M5 slice 2a/3：按序跟踪当前包段与文件段；顶层声明注册进文件段，
    // 非 private 镜像进包作用域（跨文件可见；private 仅同文件）
    let mut cur = root;
    let mut seg = root;
    let mut pkgs: std::collections::HashMap<String, ScopeId> = std::collections::HashMap::new();
    // M5 slice 2b：import 延后到全部真实声明之后注册——同包本地定义
    // 恒优先于导入别名（同包自导入、跨文件同名合法化）；导入互撞仍诊断
    let mut pending_imports: Vec<(ScopeId, &DeclNode)> = Vec::new();
    // M9 9d 根因修复：impl 注册延后到签名 lower 之后——impl 头对泛型
    // enum/struct 的急切实例化（mono spec 缓存）原在 declare 期跑，
    // 会缓存 sig.variants/params 未 lower 的空壳 spec（2026-11 审查）
    let mut pending_impls: Vec<(ScopeId, usize)> = Vec::new();
    for (i, item) in ast.items.iter().enumerate() {
        if let DeclKind::PackageSeg(path) = &item.kind {
            cur = pkg_scope(path, root, &mut pkgs, st);
            seg = st.scopes.new_scope(Some(cur)); // M5 slice 3：文件段
            st.decl_def.push(None); // 保持 decl_def 与 items 1:1
            continue;
        }
        if matches!(item.kind, DeclKind::Import(_)) {
            st.decl_def.push(None);
            // M5 slice 3：导入别名注册进文件段作用域（逐文件隔离；
            // 包级共享会让两个文件 import 同名互撞）
            pending_imports.push((seg, item));
            continue;
        }
        if matches!(item.kind, DeclKind::Impl(_)) {
            st.decl_def.push(None);
            pending_impls.push((seg, i));
            continue;
        }
        let vis = vis_of(item, st);
        declare_one(item, i, seg, cur, vis, st);
    }
    for (scope, item) in pending_imports {
        register_import(scope, item, st);
    }
    lower_top_level_sigs(ast, st);
    for (scope, index) in pending_impls {
        register_impl(ast, &ast.items[index], index, scope, st);
    }
}

/// M5 slice 2b：注册导入别名。同名真实定义已占位（本地优先）则跳过；
/// 既有导入别名则维持 SemDuplicateDef。
fn register_import(scope: ScopeId, decl: &DeclNode, st: &mut SemState) {
    let DeclKind::Import(i) = &decl.kind else {
        return;
    };
    if let ImportKind::Qualified { path, suffix } = &i.kind {
        let segments: Vec<Symbol> = path.segments.iter().map(|s| s.name).collect();
        let name = match suffix {
            ImportSuffix::Alias(a) => *a,
            _ => match segments.last() {
                Some(s) => *s,
                None => return,
            },
        };
        if matches!(suffix, ImportSuffix::Star) {
            return;
        }
        // 链上（文件段 → 包 → 根）真实定义恒优先于导入别名
        let mut s = Some(scope);
        while let Some(x) = s {
            if let Some(d) = st.scopes.lookup_local(x, name)
                && !matches!(st.defs.get(d).kind, DefKind::ImportNs { .. })
            {
                return;
            }
            s = st.scopes.parent_of(x);
        }
        // M5 slice 3：internal 目标跨包拒收（急切解析目标做可见性判定）
        let root = st.scopes.root();
        let mut target = lookup_local_no_import(st, root, segments[0]);
        for seg in &segments[1..] {
            target = target.and_then(|d| member_lookup(st, d, *seg));
        }
        if target.is_none() {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUndefinedName,
                decl.span,
                "import target not found",
            ));
            return;
        }
        if target.is_none()
            && st
                .defs
                .iter()
                .any(|def| def.name == name && def.sig.vis == Vis::Private)
        {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                decl.span,
                format!(
                    "`{}` is not visible here (private to another file)",
                    st.interner.resolve(name)
                ),
            ));
            return;
        }
        if let Some(t) = target
            && !vis_allows(st, t, scope)
        {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                decl.span,
                format!(
                    "`{}` is not visible here (internal to another package)",
                    st.interner.resolve(st.defs.get(t).name)
                ),
            ));
            return;
        }
        add_def(st, scope, name, decl.span, DefKind::ImportNs { segments });
    }
    // Native import：v1 注册别名名（调用解析为 unsupported）
}

/// M9 9d：impl 注册（延后到签名 lower 后；泛型 enum/struct 实例化
/// 须先有代入前的 sig）。重复 impl 拒收；签名与 trait 逐一核对。
/// Trait methods plus inherited supertrait methods, deterministic by
/// traversal order and tolerant of declaration-phase scope sharing.
fn trait_methods_with_inherited(st: &SemState, def: DefId) -> Vec<(ast::interner::Symbol, DefId)> {
    let mut out: Vec<(ast::interner::Symbol, DefId)> = Vec::new();
    let mut seen: Vec<DefId> = Vec::new();
    let mut queue = vec![def];
    while let Some(cur) = queue.pop() {
        if seen.contains(&cur) {
            continue;
        }
        seen.push(cur);
        queue.extend(st.defs.get(cur).sig.supertypes.iter().rev().copied());
        let Some(ms) = st.defs.get(cur).member_scope else {
            continue;
        };
        for (name, method) in st.scopes.entries(ms) {
            if matches!(st.defs.get(method).kind, DefKind::Fun)
                && !out.iter().any(|(n, _)| *n == name)
            {
                out.push((name, method));
            }
        }
    }
    out
}

/// Find a method already supplied by an impl of `def` or one of its
/// transitive supertraits for `type_def`.
fn inherited_trait_method(
    st: &SemState,
    def: DefId,
    type_def: DefId,
    name: ast::interner::Symbol,
) -> Option<DefId> {
    for parent in st.defs.get(def).sig.supertypes.clone() {
        if let Some(im) = st
            .impls
            .iter()
            .find(|im| im.trait_def == parent && im.type_def == type_def)
            && let Some((_, method)) = im.methods.iter().find(|(n, _)| *n == name)
        {
            return Some(*method);
        }
        if let Some(method) = inherited_trait_method(st, parent, type_def, name) {
            return Some(method);
        }
    }
    None
}

/// Locate a trait default-body method def by declared method name.
fn trait_default_method(
    st: &SemState,
    ast: &SourceFile,
    name: ast::interner::Symbol,
) -> Option<DefId> {
    for (idx, item) in ast.items.iter().enumerate() {
        let DeclKind::Trait(t) = &item.kind else {
            continue;
        };
        for member in &t.members {
            if let ast::syntax::decl::TraitMember::Fun(f) = member
                && f.name == name
                && f.body.is_some()
            {
                let Some(tdef) = st.decl_def.get(idx).copied().flatten() else {
                    continue;
                };
                return member_lookup(st, tdef, name);
            }
        }
    }
    None
}

fn register_impl(
    ast: &SourceFile,
    decl: &DeclNode,
    index: usize,
    scope: ScopeId,
    st: &mut SemState,
) {
    let DeclKind::Impl(i) = &decl.kind else {
        return;
    };
    // M9 9a：impl 注册（Q4：v1 不限 orphan——单 crate whole-program；
    // 重复 impl 拒收；泛型 impl 9d 后置；成员限函数；签名与 trait
    // 逐一核对，Self 代入后类型相等）
    st.in_impl_header = true;
    // 注意字段名与语法位相反：parser 把 for 前的 trait 位存进
    // type_ref、for 后的类型位存进 trait_ref（ ImplDecl 命名如此）
    let iscope = if i.type_params.is_empty() {
        scope
    } else {
        type_param_scope(st, scope, &i.type_params)
    };
    let trait_ty = lower_ty(st, scope, &i.type_ref);
    let type_ty = lower_ty(st, iscope, &i.trait_ref);
    st.in_impl_header = false;
    // P0 compiler trust：impl 头类型实参必须是本 impl 绑定的形参或具体
    // 类型。未绑定 Param / trait 名充当类型实参此前会静默注册、调用点
    // 误报「method calls are not supported」；声明点稳定拒收。
    {
        let mut params: Vec<u32> = Vec::new();
        st.tys.collect_param_indices(type_ty, &mut params);
        st.tys.collect_param_indices(trait_ty, &mut params);
        params.sort_unstable();
        params.dedup();
        let bound = i.type_params.len() as u32;
        if params.iter().any(|p| *p >= bound) {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                i.trait_ref.span,
                "impl type parameter is not bound; write impl<T> ...",
            ));
            return;
        }
        if impl_type_arg_is_trait(st, type_ty) {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                i.trait_ref.span,
                "trait cannot be used as a type argument",
            ));
            return;
        }
    }
    let Some(tdef) = (match st.tys.kind(trait_ty) {
        TyKind::Named(d, _) if matches!(st.defs.get(*d).kind, DefKind::Trait) => Some(*d),
        _ => None,
    }) else {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemTypeMismatch,
            i.trait_ref.span,
            "impl header must name a trait",
        ));
        return;
    };
    let Some(tydef) = (match st.tys.kind(type_ty) {
        TyKind::Named(d, _) => Some(*d),
        _ => None,
    }) else {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemTypeMismatch,
            i.type_ref.span,
            "impl header must name a nominal type",
        ));
        return;
    };
    if st
        .impls
        .iter()
        .any(|im| im.trait_def == tdef && im.type_def == tydef)
    {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemDuplicateDef,
            decl.span,
            "duplicate impl for this trait and type",
        ));
        return;
    }
    let Some(tms) = st.defs.get(tdef).member_scope else {
        return;
    };
    let self_sym = st.interner.intern("Self");
    let Some(self_def) = st.scopes.lookup_local(tms, self_sym) else {
        return;
    };
    // impl 方法注册进独立作用域（不污染段命名空间；经注册表解析）
    let ims = st.scopes.new_scope(Some(scope));
    let mut methods: Vec<(Symbol, DefId)> = Vec::new();
    for member in &i.members {
        let ast::syntax::decl::ClassMember::Declaration(d) = member else {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                decl.span,
                "impl members must be function definitions",
            ));
            continue;
        };
        let ast::syntax::decl::DeclKind::Fun(f) = &d.kind else {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                decl.span,
                "impl members must be function definitions",
            ));
            continue;
        };
        let fd = add_def(st, ims, f.name, d.span, DefKind::Fun);
        let params = lower_params(st, iscope, &f.params, d.span);
        let ret = match &f.ret {
            Some(r) => lower_ty(st, scope, r),
            None => st.tys.unit,
        };
        let defaults: Vec<bool> = f.params.iter().map(|p| p.default.is_some()).collect();
        let s = &mut st.defs.get_mut(fd).sig;
        s.params = params;
        s.ret = ret;
        s.param_defaults = defaults;
        st.defs.get_mut(fd).decl = Some(index);
        methods.push((f.name, fd)); // Impl methods receive the concrete receiver, not a trait Self
        // placeholder; this keeps direct and vtable ABI identical.
        st.defs.get_mut(fd).sig.ty = type_ty;
    }
    // 覆盖与签名核对：trait 每个签名须恰有一个同名同型的 impl 方法
    let trait_methods: Vec<(Symbol, DefId)> = trait_methods_with_inherited(st, tdef);
    // Missing methods may come from a trait default body; the method def is
    // reused by every impl so static dispatch stays a direct call.
    for (name, _) in &trait_methods {
        if methods.iter().any(|(n, _)| n == name) {
            continue;
        }
        if let Some(default_fd) = trait_default_method(st, ast, *name) {
            methods.push((*name, default_fd));
        }
    }
    // A supertrait impl satisfies inherited requirements. Keep the derived
    // registry free of copied methods so dispatch remains unambiguous: the
    // most specific impl for each method owns that method.
    for (name, _) in &trait_methods {
        if methods.iter().any(|(n, _)| n == name) {
            continue;
        }
        if inherited_trait_method(st, tdef, tydef, *name).is_none() {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnknownMember,
                decl.span,
                format!(
                    "impl is missing inherited trait method `{}`",
                    st.interner.resolve(*name)
                ),
            ));
        }
    }
    for (name, tfd) in &trait_methods {
        match methods.iter().find(|(n, _)| n == name) {
            // Missing here is either reported by the inherited-requirement
            // pass above or supplied by a supertrait impl; do not duplicate.
            None => {}
            Some((_, ifd)) => {
                let (tparams, tret) = {
                    let s = &st.defs.get(*tfd).sig;
                    (s.params.clone(), s.ret)
                };
                let (iparams, iret) = {
                    let s = &st.defs.get(*ifd).sig;
                    (s.params.clone(), s.ret)
                };
                let ok_arity = tparams.len() == iparams.len();
                let ok_params = ok_arity
                    && tparams
                        .iter()
                        .zip(iparams.iter())
                        .all(|((_, tt), (_, it))| subst_self(st, *tt, self_def, type_ty) == *it);
                let ok_ret = subst_self(st, tret, self_def, type_ty) == iret;
                if !ok_arity || !ok_params || !ok_ret {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemTypeMismatch,
                        decl.span,
                        format!(
                            "impl method `{}` does not match the trait signature",
                            st.interner.resolve(*name)
                        ),
                    ));
                }
            }
        }
    }
    for (name, _) in &methods {
        if !trait_methods.iter().any(|(n, _)| n == name) {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnknownMember,
                decl.span,
                format!(
                    "impl method `{}` is not declared by the trait",
                    st.interner.resolve(*name)
                ),
            ));
        }
    }
    st.impls.push(crate::def::TraitImpl {
        trait_def: tdef,
        type_def: tydef,
        span: decl.span,
        methods,
    });
}

/// M5 slice 3：作用域的包身份链（祖先中的包作用域，根→叶序）
pub(crate) fn pkg_chain(st: &SemState, scope: crate::def::ScopeId) -> Vec<crate::def::ScopeId> {
    let mut v = Vec::new();
    let mut cur = Some(scope);
    while let Some(s) = cur {
        if st.pkg_scopes.contains(&s) {
            v.push(s);
        }
        cur = st.scopes.parent_of(s);
    }
    v
}

/// M5 slice 3：两个作用域是否同属一个包身份链
pub(crate) fn same_pkg(st: &SemState, a: crate::def::ScopeId, b: crate::def::ScopeId) -> bool {
    pkg_chain(st, a) == pkg_chain(st, b)
}

/// M5 slice 3：internal 目标的跨包使用拒收判定（true = 允许）
pub(crate) fn vis_allows(st: &SemState, def: DefId, use_scope: crate::def::ScopeId) -> bool {
    match st.defs.get(def).sig.vis {
        crate::def::Vis::Pub => true,
        crate::def::Vis::Internal => same_pkg(st, st.defs.get(def).scope, use_scope),
        crate::def::Vis::Private => {
            // private 注册在文件段作用域：仅同段解析链可达（链上自然拒收）；
            // 经命名空间/导入命中属契约破缺，按同包口径放行诊断兜底
            same_pkg(st, st.defs.get(def).scope, use_scope)
        }
    }
}

/// M5 slice 2a：取（必要时建）dotted 包路径的作用域。每段注册 PackageNs
/// def（member_scope = 该包顶层作用域），末段作用域即声明目标。
fn pkg_scope(
    path: &ast::syntax::node::Path,
    root: ScopeId,
    pkgs: &mut std::collections::HashMap<String, ScopeId>,
    st: &mut SemState,
) -> ScopeId {
    let key: Vec<String> = path
        .segments
        .iter()
        .map(|s| st.interner.resolve(s.name).to_string())
        .collect();
    if let Some(s) = pkgs.get(&key.join(".")) {
        return *s;
    }
    let mut cur = root;
    let mut acc = String::new();
    for seg in &path.segments {
        if !acc.is_empty() {
            acc.push('.');
        }
        acc.push_str(st.interner.resolve(seg.name));
        if let Some(&cached) = pkgs.get(&acc) {
            cur = cached;
            continue;
        }
        let scope = st.scopes.new_scope(Some(cur));
        st.pkg_scopes.push(scope);
        let def = st.defs.push(Def {
            kind: DefKind::PackageNs,
            name: seg.name,
            scope: cur,
            span: seg.span,
            sig: DefSig::new(st.tys.unit, st.tys.unknown),
            resolving: false,
            alias_of: None,
            alias_resolved: None,
            member_scope: Some(scope),
            external: false,
            ty_params: 0,
            decl: None,
        });
        if let Some(old) = st.scopes.insert(cur, seg.name, def) {
            st.scopes.insert(cur, seg.name, old);
            st.diags.emit(Diagnostic::error(
                DiagCode::SemDuplicateDef,
                seg.span,
                format!(
                    "duplicate definition of `{}`",
                    st.interner.resolve(seg.name)
                ),
            ));
            // 复用既有 def 的作用域，保持同包同名合并
            cur = st.defs.get(old).member_scope.unwrap_or(cur);
            pkgs.insert(acc.clone(), cur);
            continue;
        }
        cur = scope;
        pkgs.insert(acc.clone(), cur);
    }
    cur
}

/// M7 增量入口：只处理克隆追加的 items[st.items_done..]。
/// 不重跑 register_builtins/lower_top_level_sigs（它们只服务原件）；
/// 克隆已在 mono::make_fun_clone 里登记（spec def + 类型形参作用域），
/// 这里跳过声明（再 add_def 会与原件撞 SemDuplicateDef）。
pub(crate) fn run_from(ast: &SourceFile, st: &mut SemState, start: usize) {
    let root = st.scopes.root();
    for (i, item) in ast.items.iter().enumerate().skip(start) {
        if i < st.decl_def.len() {
            continue;
        }
        let _ = (item, i, root);
    }
}

/// 逐项声明：先占 decl_def 槽（与 items 1:1 对齐），再登记名字
fn declare_one(
    decl: &DeclNode,
    index: usize,
    seg: ScopeId,
    pkg: ScopeId,
    vis: crate::def::Vis,
    st: &mut SemState,
) {
    st.decl_def.push(None);
    declare_decl(decl, index, seg, st);
    // M5 slice 3：非 private 声明镜像进包作用域（同包跨文件可见）；
    // 镜像撞名 = 同包公开同名，报 SemDuplicateDef
    if vis != crate::def::Vis::Private && seg != pkg {
        let def = st
            .decl_def
            .last()
            .copied()
            .flatten()
            .or_else(|| top_binding_def(decl, st));
        if let Some(def) = def {
            st.defs.get_mut(def).sig.vis = vis;
            if let Some(old) = st.scopes.insert(pkg, st.defs.get(def).name, def) {
                st.scopes.insert(pkg, st.defs.get(def).name, old);
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemDuplicateDef,
                    decl.span,
                    format!(
                        "duplicate definition of `{}`",
                        st.interner.resolve(st.defs.get(def).name)
                    ),
                ));
            }
        }
    }
}

/// M5 slice 3：顶层绑定的 def 经 pat_binding_def 侧链（declare_binding_name
/// 不填 decl_def 槽）
fn top_binding_def(decl: &DeclNode, st: &SemState) -> Option<DefId> {
    let DeclKind::Binding(b) = &decl.kind else {
        return None;
    };
    st.pat_binding_def.get(b.pattern.id).copied()
}

/// M5 slice 3：顶层声明可见性提取（首个可见性修饰词胜出；protected
/// 为 class 成员专用，顶层使用稳定拒收后按 Internal 处理）
fn vis_of(decl: &DeclNode, st: &mut SemState) -> crate::def::Vis {
    use ast::token::ModifierWord;
    let mods: &[ast::syntax::modifier::Modifier] = match &decl.kind {
        DeclKind::Fun(f) => &f.modifiers.modifiers,
        DeclKind::Binding(b) => &b.modifiers.modifiers,
        DeclKind::Class(c) => &c.modifiers.modifiers,
        DeclKind::Struct(s) => &s.modifiers.modifiers,
        DeclKind::Enum(e) => &e.modifiers.modifiers,
        DeclKind::TypeAlias(t) => &t.modifiers.modifiers,
        DeclKind::Extern(e) => match &e.body {
            ExternBody::Fun(sig) => &sig.modifiers.modifiers,
            _ => &[],
        },
        _ => &[],
    };
    reject_unsupported_modifiers(mods, st);
    for m in mods {
        match m.word {
            ModifierWord::Private => return crate::def::Vis::Private,
            ModifierWord::Internal => return crate::def::Vis::Internal,
            ModifierWord::Public => return crate::def::Vis::Pub,
            ModifierWord::Protected => {
                // P0 compiler trust：继承未实现，protected 无可定义跨类语义
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnsupportedConstruct,
                    decl.span,
                    "`protected` is not supported until inheritance is implemented",
                ));
                return crate::def::Vis::Internal;
            }
            _ => {}
        }
    }
    crate::def::Vis::Pub
}

/// P0 compiler trust：无实现路径的修饰词在声明点稳定拒收。
/// public/private/internal/weak 有语义；protected 由 vis_of/class 成员
/// 路径单独处理；其余一律拒收，杜绝静默接受。
fn decl_flags(mods: &[ast::syntax::modifier::Modifier]) -> crate::def::DeclFlags {
    use ast::token::ModifierWord;
    let mut flags = crate::def::DeclFlags::default();
    for m in mods {
        match m.word {
            ModifierWord::Open => flags.open = true,
            ModifierWord::Abstract => flags.abstract_ = true,
            ModifierWord::Sealed => flags.sealed = true,
            ModifierWord::Override => flags.overrides = true,
            ModifierWord::Protected => flags.protected = true,
            _ => {}
        }
    }
    flags
}

fn reject_unsupported_modifiers(mods: &[ast::syntax::modifier::Modifier], st: &mut SemState) {
    use ast::token::ModifierWord;
    for m in mods {
        let msg: &'static str = match m.word {
            ModifierWord::OperatorWord => "operator declarations are not supported in v1",
            _ => continue,
        };
        st.diags.emit(Diagnostic::error(
            DiagCode::SemUnsupportedConstruct,
            m.span,
            msg,
        ));
    }
}

/// P0 compiler trust：impl 头 for-类型的泛型实参里是否出现 trait
///（根位置 trait 已由「impl header must name a nominal type」拒收）。
fn impl_type_arg_is_trait(st: &SemState, ty: TyId) -> bool {
    match st.tys.kind(ty) {
        TyKind::Named(_, args) => {
            args.iter().any(|a| {
                matches!(
                    st.tys.kind(*a),
                    TyKind::Named(d, _) if matches!(st.defs.get(*d).kind, DefKind::Trait)
                )
            }) || args.iter().any(|a| impl_type_arg_is_trait(st, *a))
        }
        TyKind::Nullable(i) | TyKind::Range(i) => impl_type_arg_is_trait(st, *i),
        TyKind::Array(e, _) => impl_type_arg_is_trait(st, *e),
        TyKind::Tuple(elems) => elems.iter().any(|e| impl_type_arg_is_trait(st, *e)),
        TyKind::Fn(ps, ret) => {
            ps.iter().any(|p| impl_type_arg_is_trait(st, *p)) || impl_type_arg_is_trait(st, *ret)
        }
        TyKind::Ref { inner, .. } | TyKind::Pointer { inner, .. } => {
            impl_type_arg_is_trait(st, *inner)
        }
        _ => false,
    }
}

fn register_builtins(st: &mut SemState) {
    let root = st.scopes.root();
    let zero = Span { start: 0, end: 0 };
    for name in BUILTINS {
        let sym = st.interner.intern(name);
        let def = st.defs.push(Def {
            kind: DefKind::BuiltinType,
            name: sym,
            scope: root,
            span: zero,
            sig: DefSig::new(st.tys.unit, st.tys.unknown),
            resolving: false,
            alias_of: None,
            alias_resolved: None,
            member_scope: None,
            external: false,
            ty_params: 0,
            decl: None,
        });
        st.scopes.insert(root, sym, def);
        if *name == "Own" {
            st.own_def = Some(def);
        }
    }
}

fn add_def(st: &mut SemState, scope: ScopeId, name: Symbol, span: Span, kind: DefKind) -> DefId {
    let def = st.defs.push(Def {
        kind,
        name,
        scope,
        span,
        sig: DefSig::new(st.tys.unit, st.tys.unknown),
        resolving: false,
        alias_of: None,
        alias_resolved: None,
        member_scope: None,
        external: false,
        ty_params: 0,
        decl: None,
    });
    if let Some(old) = st.scopes.insert(scope, name, def) {
        // 保留第一个定义，重复项报错
        st.scopes.insert(scope, name, old);
        st.diags.emit(Diagnostic::error(
            DiagCode::SemDuplicateDef,
            span,
            format!("duplicate definition of `{}`", st.interner.resolve(name)),
        ));
    }
    def
}

/// M4 slice 1：class 字段的 8B 标量判据（与 irc::layout::is_aggregate 同口径的
/// sem 侧镜像——sem 不依赖 irc，种类表手工对齐；i128 单独拒）
fn class_field_ok(tys: &TyArena, ty: TyId) -> bool {
    // i128 单独拒：payload 槽模型 slice 1 只保 8B 槽（guard 不跨 or 臂）
    match tys.kind(ty) {
        TyKind::Int(IntKind::I128) => false,
        TyKind::Int(_)
        | TyKind::Float(_)
        | TyKind::Bool
        | TyKind::Char
        | TyKind::String
        | TyKind::Ref { .. }
        | TyKind::Pointer { .. } => true,
        _ => false,
    }
}

pub(crate) fn is_class_ty(st: &SemState, ty: TyId) -> bool {
    matches!(
        st.tys.kind(ty),
        TyKind::Named(d, _) if matches!(st.defs.get(*d).kind, DefKind::Class)
    )
}

/// M7：类型形参作用域——形参名注册为 DefKind::TypeParam，返回新作用域
/// （空列表返回 parent 自身）。泛型 fun 把它存 member_scope 供
/// resolve/check 期作 pscope 祖先；struct 只作签名 lower 的临时作用域。
fn type_param_scope(
    st: &mut SemState,
    parent: ScopeId,
    type_params: &[ast::syntax::ty::TypeParam],
) -> ScopeId {
    let scope = st.scopes.new_scope(Some(parent));
    for (i, tp) in type_params.iter().enumerate() {
        let ast::syntax::ty::TypeParamName::Valid(name) = &tp.name else {
            continue;
        };
        let def = st.defs.push(Def {
            kind: DefKind::TypeParam { index: i as u32 },
            name: *name,
            scope,
            span: tp.span,
            sig: DefSig::new(st.tys.unit, st.tys.unknown),
            resolving: false,
            alias_of: None,
            alias_resolved: None,
            member_scope: None,
            external: false,
            ty_params: 0,
            decl: None,
        });
        // 同名形参：保留首个，重复项报错（与参数重名同纪律）
        if let Some(old) = st.scopes.insert(scope, *name, def) {
            st.scopes.insert(scope, *name, old);
            st.diags.emit(Diagnostic::error(
                DiagCode::SemDuplicateDef,
                tp.span,
                format!("duplicate type parameter `{}`", st.interner.resolve(*name)),
            ));
        }
    }
    scope
}

/// M7 slice 1 拒收面：类型形参的 bounds/默认值/variance 与 where 子句。
/// （trait bounds 是 M9 的地基，先给稳定诊断而非半特性。）
fn reject_generic_extras(
    type_params: &[ast::syntax::ty::TypeParam],
    _where_clause: &[WhereConstraint],
    span: Span,
    st: &mut SemState,
    what: &str,
) {
    // M7 slice 2 + M9 9c：类型形参默认值与 bounds 已落地；variance/
    // where 仍拒收（where 是 bounds 的语法糖后置）
    let fancy = type_params.iter().any(|tp| tp.variance.is_some());
    if fancy {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemUnsupportedConstruct,
            span,
            format!(
                "type parameter bounds/defaults/variance on generic {what} are not supported in this slice"
            ),
        ));
    }
}
/// M9 9a：把 trait 签名里的 Self（Named(self_def)）替换为 impl 目标类型
fn subst_self(st: &mut SemState, ty: TyId, self_def: DefId, target: TyId) -> TyId {
    match st.tys.kind(ty).clone() {
        TyKind::Named(d, _) if d == self_def => target,
        TyKind::Named(d, args) => {
            let args: Vec<TyId> = args
                .iter()
                .map(|a| subst_self(st, *a, self_def, target))
                .collect();
            st.tys.intern(TyKind::Named(d, args))
        }
        TyKind::Nullable(i) => {
            let i = subst_self(st, i, self_def, target);
            st.tys.intern(TyKind::Nullable(i))
        }
        TyKind::Array(e, n) => {
            let e = subst_self(st, e, self_def, target);
            st.tys.intern(TyKind::Array(e, n))
        }
        TyKind::Ref { mutable, inner } => {
            let inner = subst_self(st, inner, self_def, target);
            st.tys.intern(TyKind::Ref { mutable, inner })
        }
        TyKind::Pointer { mutable, inner } => {
            let inner = subst_self(st, inner, self_def, target);
            st.tys.intern(TyKind::Pointer { mutable, inner })
        }
        TyKind::Range(i) => {
            let i = subst_self(st, i, self_def, target);
            st.tys.intern(TyKind::Range(i))
        }
        TyKind::Fn(ps, r) => {
            let ps: Vec<TyId> = ps
                .iter()
                .map(|p| subst_self(st, *p, self_def, target))
                .collect();
            let r = subst_self(st, r, self_def, target);
            st.tys.intern(TyKind::Fn(ps, r))
        }
        _ => ty,
    }
}

fn declare_decl(decl: &DeclNode, index: usize, scope: ScopeId, st: &mut SemState) {
    match &decl.kind {
        // M5 slice 2a：段标记在 run() 主循环消费（保持 decl_def 1:1），此处无操作
        DeclKind::PackageSeg(_) => {}
        DeclKind::Import(_i) => {
            // M5 slice 2b：导入别名在 run() 第二遍注册（本地定义优先）；
            // 此处无操作（Native import 仍由调用点报 unsupported）
        }
        DeclKind::Fun(f) => {
            let def = add_def(st, scope, f.name, decl.span, DefKind::Fun);
            st.defs.get_mut(def).ty_params = f.type_params.len() as u32;
            st.defs.get_mut(def).decl = Some(index);
            *st.decl_def.last_mut().expect("decl_def slot") = Some(def);
            if !f.type_params.is_empty() {
                reject_generic_extras(&f.type_params, &f.where_clause, decl.span, st, "functions");
                if st.interner.resolve(f.name) == "main" {
                    // 入口无法实例化：泛型 main 永远无体可链
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemUnsupportedConstruct,
                        decl.span,
                        "generic `main` is not supported (entry point cannot be instantiated)",
                    ));
                }
                // 类型形参作用域：签名（declare 期）与体（resolve/check 期，
                // 经 member_scope 作 pscope 祖先）共用；M7 单态化克隆各建一份
                let tscope = type_param_scope(st, scope, &f.type_params);
                st.defs.get_mut(def).member_scope = Some(tscope);
                // M7 slice 2：类型形参默认值 lower 进签名（tscope 内
                // 可引用早先形参，落成 TyKind::Param 由调用点代入）
                let tp_defaults: Vec<Option<TyId>> = f
                    .type_params
                    .iter()
                    .map(|tp| tp.default.as_ref().map(|d| lower_ty(st, tscope, d)))
                    .collect();
                st.defs.get_mut(def).sig.tp_defaults = tp_defaults;
                // M9 9c：bounds 解析为 trait def 列表（tscope 内解析）
                st.in_impl_header = true; // bounds 命名 trait 合法
                let tp_bounds: Vec<Vec<DefId>> = f
                    .type_params
                    .iter()
                    .map(|tp| {
                        tp.bounds
                            .iter()
                            .filter_map(|b| {
                                let bt = lower_ty(st, tscope, b);
                                match st.tys.kind(bt) {
                                    TyKind::Named(d, _)
                                        if matches!(st.defs.get(*d).kind, DefKind::Trait) =>
                                    {
                                        Some(*d)
                                    }
                                    _ => {
                                        st.diags.emit(Diagnostic::error(
                                            DiagCode::SemTypeMismatch,
                                            b.span,
                                            "type parameter bounds must name traits",
                                        ));
                                        None
                                    }
                                }
                            })
                            .collect()
                    })
                    .collect();
                st.in_impl_header = false;
                st.defs.get_mut(def).sig.tp_bounds = tp_bounds;
            }
        }
        DeclKind::Binding(b) => {
            declare_binding_name(b, scope, decl.span, st);
        }
        DeclKind::Struct(s) => {
            let def = add_def(st, scope, s.name, decl.span, DefKind::Struct);
            st.defs.get_mut(def).ty_params = s.type_params.len() as u32;
            st.defs.get_mut(def).decl = Some(index);
            *st.decl_def.last_mut().expect("decl_def slot") = Some(def);
            if !s.type_params.is_empty() {
                reject_generic_extras(&s.type_params, &[], decl.span, st, "structs");
                if s.primary_ctor.iter().any(|p| p.default.is_some()) {
                    // 默认值表达式逐实例化检查未排期（M7 后续切片）
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemUnsupportedConstruct,
                        decl.span,
                        "generic struct primary-constructor defaults are not supported in this slice",
                    ));
                }
            }
            let ms = st.scopes.new_scope(Some(scope));
            st.defs.get_mut(def).member_scope = Some(ms);
            // M3 字段注册序 = 槽布局序：头构造 val/var 参数（声明字段）在前，
            // body 字段在后；member_scope 内重名由 add_def 报 SemDuplicateDef。
            let mut index = 0usize;
            for p in &s.primary_ctor {
                if matches!(
                    p.modifier,
                    ast::syntax::decl::ParamModifier::Val | ast::syntax::decl::ParamModifier::Var
                ) {
                    add_def(
                        st,
                        ms,
                        p.name,
                        decl.span,
                        DefKind::StructField {
                            parent: def,
                            index,
                            mutable: Some(p.modifier == ast::syntax::decl::ParamModifier::Var),
                        },
                    );
                    index += 1;
                }
            }
            for field in &s.fields {
                // 显式 var/val 走三态；无修饰符 = None（legacy：按根绑定可变性）
                let mutable = field.mutable;
                add_def(
                    st,
                    ms,
                    field.name,
                    decl.span,
                    DefKind::StructField {
                        parent: def,
                        index,
                        mutable,
                    },
                );
                index += 1;
            }
        }
        DeclKind::Enum(e) => {
            let def = add_def(st, scope, e.name, decl.span, DefKind::Enum);
            st.defs.get_mut(def).ty_params = e.type_params.len() as u32;
            st.defs.get_mut(def).decl = Some(index);
            *st.decl_def.last_mut().expect("decl_def slot") = Some(def);
            let ms = st.scopes.new_scope(Some(scope));
            st.defs.get_mut(def).member_scope = Some(ms);
            for (index, variant) in e.variants.iter().enumerate() {
                add_def(
                    st,
                    ms,
                    variant.name,
                    decl.span,
                    DefKind::EnumVariant { parent: def, index },
                );
            }
        }
        DeclKind::Class(c) => {
            // M4 slice 1：class 落地（主构造 val/var 头参数=字段、堆分配、
            // 复制 retain、块出口隐式 release）。方法/继承/deinit/weak/
            // 引用相等/可空 class/二级构造 后续切片；注册保持防级联。
            let def = add_def(st, scope, c.name, decl.span, DefKind::Class);
            st.defs.get_mut(def).decl = Some(index);
            if let Some(slot) = st.decl_def.last_mut() {
                *slot = Some(def);
            }
            st.defs.get_mut(def).sig.flags = decl_flags(&c.modifiers.modifiers);
            if c.supertypes
                .iter()
                .any(|super_item| super_item.by.is_some())
            {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnsupportedConstruct,
                    decl.span,
                    "class delegation (`by`) is not supported in Phase 3a",
                ));
            }
            if c.supertypes.len() > 1 {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnsupportedConstruct,
                    decl.span,
                    "class inheritance supports at most one supertype",
                ));
            }
            let ms = st.scopes.new_scope(Some(scope));
            st.defs.get_mut(def).member_scope = Some(ms);
            // 头字段注册（M3 三态纪律同 struct 主构造）
            let mut fidx = 0usize;
            if let Some(pc) = &c.primary_ctor {
                for p in &pc.params {
                    if matches!(
                        p.modifier,
                        ast::syntax::decl::ParamModifier::Val
                            | ast::syntax::decl::ParamModifier::Var
                    ) {
                        add_def(
                            st,
                            ms,
                            p.name,
                            decl.span,
                            DefKind::StructField {
                                parent: def,
                                index: fidx,
                                mutable: Some(p.modifier == ast::syntax::decl::ParamModifier::Var),
                            },
                        );
                        fidx += 1;
                    }
                }
            }
            for member in &c.members {
                match member {
                    // M4 slice 2b：class body 绑定（字段声明）未排期——
                    // 字段走主构造 val/var 头参数（M3 纪律）
                    ClassMember::Declaration(d) if matches!(&d.kind, DeclKind::Binding(_)) => {
                        if let DeclKind::Binding(b) = &d.kind {
                            reject_unsupported_modifiers(&b.modifiers.modifiers, st);
                        }
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            d.span,
                            "class body field bindings are not supported in this slice (use primary-constructor val/var parameters)",
                        ));
                    }
                    ClassMember::Declaration(d) if matches!(&d.kind, DeclKind::Fun(_)) => {
                        if let DeclKind::Fun(f) = &d.kind {
                            reject_unsupported_modifiers(&f.modifiers.modifiers, st);
                            if f.params.iter().any(|p| p.default.is_some()) {
                                st.diags.emit(Diagnostic::error(
                                    DiagCode::SemUnsupportedConstruct,
                                    d.span,
                                    "method default parameters are not supported in this slice",
                                ));
                            }
                            add_def(st, ms, f.name, d.span, DefKind::Fun);
                        }
                    }
                    // M4 slice 2b：deinit 注册为 member_scope 里的 Fun def
                    //（名 "deinit"、零形参、unit 返回；release 经 header meta 派发）
                    ClassMember::Deinit(_) => {
                        let dname = st.interner.intern("deinit");
                        add_def(st, ms, dname, decl.span, DefKind::Fun);
                    }
                    // R4：v1 无二级构造——init 块/构造器稳定拒收
                    ClassMember::Constructor(_) | ClassMember::Init(_) => {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            decl.span,
                            "secondary constructors (init) are not supported in this slice",
                        ));
                    }
                    // P0 compiler trust：accessor 无属性语义——此前被静默忽略
                    ClassMember::Accessor(a) => {
                        let span = a
                            .getter
                            .as_ref()
                            .or(a.setter.as_ref())
                            .and_then(|b| b.stmts.first().map(|s| s.span))
                            .unwrap_or(decl.span);
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            span,
                            "accessors are not supported in v1",
                        ));
                    }
                    _ => {}
                }
            }
        }
        DeclKind::TypeAlias(t) => {
            let def = add_def(st, scope, t.name, decl.span, DefKind::TypeAlias);
            st.defs.get_mut(def).ty_params = t.type_params.len() as u32;
            *st.decl_def.last_mut().expect("decl_def slot") = Some(def);
            st.defs.get_mut(def).alias_of = Some(t.ty.clone());
        }
        DeclKind::Extern(e) => {
            // M5 slice 2a：decl_def 填首个成员 def（签名 lower 经它取包作用域）
            let mut first = None;
            match &e.body {
                ExternBody::Fun(sig) => {
                    let def = add_def(st, scope, sig.name, decl.span, DefKind::Fun);
                    st.defs.get_mut(def).external = true;
                    first = Some(def);
                }
                ExternBody::Block { members } => {
                    for sig in members {
                        let def = add_def(st, scope, sig.name, decl.span, DefKind::Fun);
                        st.defs.get_mut(def).external = true;
                        if first.is_none() {
                            first = Some(def);
                        }
                    }
                }
            }
            if let Some(d) = first {
                *st.decl_def.last_mut().expect("decl_def slot") = Some(d);
            }
        }
        DeclKind::Union(u) => {
            // P0 compiler trust：union 无构造/字段访问实现——此前声明被
            // 静默注册（冒充 Struct）、构造点才报错；声明点稳定拒收。
            let _ = u;
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                decl.span,
                "union declarations are not supported in v1",
            ));
        }
        DeclKind::Trait(t) => {
            // M9 9a/WP5: signature-only traits, now with single-path
            // inheritance. Default methods and associated types remain gated.
            let def = add_def(st, scope, t.name, decl.span, DefKind::Trait);
            st.defs.get_mut(def).ty_params = t.type_params.len() as u32;
            *st.decl_def.last_mut().expect("decl_def slot") = Some(def);
            let ms = st.scopes.new_scope(Some(scope));
            st.defs.get_mut(def).member_scope = Some(ms);
            let tscope = type_param_scope(st, ms, &t.type_params);
            // Self placeholder is visible to trait signatures and replaced by
            // the impl target during registration.
            let self_sym = st.interner.intern("Self");
            let unit = st.tys.unit;
            let unknown = st.tys.unknown;
            let self_def = st.defs.push(Def {
                kind: DefKind::BuiltinType,
                name: self_sym,
                scope: ms,
                span: decl.span,
                sig: DefSig::new(unit, unknown),
                resolving: false,
                alias_of: None,
                alias_resolved: None,
                member_scope: None,
                external: false,
                ty_params: 0,
                decl: None,
            });
            st.scopes.insert(ms, self_sym, self_def);
            let mut supertypes: Vec<DefId> = Vec::new();
            st.in_impl_header = true;
            for sup in &t.supertypes {
                let ty = lower_ty(st, tscope, &sup.ty);
                if let TyKind::Named(d, _) = st.tys.kind(ty)
                    && matches!(st.defs.get(*d).kind, DefKind::Trait)
                {
                    supertypes.push(*d);
                }
            }
            st.in_impl_header = false;
            supertypes.sort();
            supertypes.dedup();
            if supertypes.len() != t.supertypes.len() {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnsupportedConstruct,
                    decl.span,
                    "trait supertypes must name traits",
                ));
            }
            let mut ancestors: Vec<DefId> = supertypes.clone();
            let mut i = 0;
            while i < ancestors.len() {
                let cur = ancestors[i];
                if st.defs.get(cur).resolving {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemTypeCycle,
                        decl.span,
                        "trait inheritance cycle",
                    ));
                    break;
                }
                for parent in &st.defs.get(cur).sig.supertypes {
                    if !ancestors.contains(parent) {
                        ancestors.push(*parent);
                    } else {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            decl.span,
                            "trait inheritance must be a single path (diamonds are rejected)",
                        ));
                    }
                }
                i += 1;
            }
            st.defs.get_mut(def).sig.supertypes = supertypes;
            for m in &t.members {
                match m {
                    ast::syntax::decl::TraitMember::Signature(sig) => {
                        let fd = add_def(st, ms, sig.name, decl.span, DefKind::Fun);
                        let params = lower_params(st, tscope, &sig.params, decl.span);
                        let ret = match &sig.ret {
                            Some(r) => lower_ty(st, tscope, r),
                            None => st.tys.unit,
                        };
                        // Precheck for a later dyn slice: only signature-only
                        // methods with concrete return types are object safe.
                        let ret_has_param = sig
                            .ret
                            .as_ref()
                            .map(|r| lower_ty(st, tscope, r))
                            .is_some_and(|ret| st.tys.mentions_param(ret));
                        if !sig.type_params.is_empty() || ret_has_param {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemObjectUnsafe,
                                decl.span,
                                format!(
                                    "trait method `{}` is not object safe",
                                    st.interner.resolve(sig.name)
                                ),
                            ));
                        }
                        let defaults: Vec<bool> =
                            sig.params.iter().map(|p| p.default.is_some()).collect();
                        let s = &mut st.defs.get_mut(fd).sig;
                        s.params = params;
                        s.ret = ret;
                        s.param_defaults = defaults;
                    }
                    ast::syntax::decl::TraitMember::Fun(f) => {
                        reject_unsupported_modifiers(&f.modifiers.modifiers, st);
                        if f.params.iter().any(|p| p.default.is_some()) {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemUnsupportedConstruct,
                                decl.span,
                                "trait method default parameters are not supported in this slice",
                            ));
                        }
                        let fd = add_def(st, ms, f.name, decl.span, DefKind::Fun);
                        let params = lower_params(st, tscope, &f.params, decl.span);
                        let ret = match &f.ret {
                            Some(r) => lower_ty(st, tscope, r),
                            None => st.tys.unit,
                        };
                        let ret_has_param = f
                            .ret
                            .as_ref()
                            .map(|r| lower_ty(st, tscope, r))
                            .is_some_and(|ret| st.tys.mentions_param(ret));
                        if !f.type_params.is_empty() || ret_has_param {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemObjectUnsafe,
                                decl.span,
                                format!(
                                    "trait method `{}` is not object safe",
                                    st.interner.resolve(f.name)
                                ),
                            ));
                        }
                        let defaults: Vec<bool> =
                            f.params.iter().map(|p| p.default.is_some()).collect();
                        let s = &mut st.defs.get_mut(fd).sig;
                        s.params = params;
                        s.ret = ret;
                        s.param_defaults = defaults;
                        // IRC injects self through the signature's self-type slot.
                        // Keep the declaration-phase placeholder concrete enough for
                        // scalar/class ABI lowering; impl dispatch passes the real type.
                        if let Some(self_def) = st.scopes.lookup_local(ms, self_sym) {
                            s.ty = st.tys.builtin_named(self_def);
                        }
                        st.defs.get_mut(fd).decl = Some(index);
                        if f.body.is_some() {
                            st.trait_defaults.entry(index).or_default().push(fd);
                        }
                    }
                    ast::syntax::decl::TraitMember::AssociatedType(_) => {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            decl.span,
                            "associated types are not supported in this slice (M9 Q8)",
                        ));
                    }
                    _ => {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            decl.span,
                            "trait default implementations are not supported in this slice (M9 Q7)",
                        ));
                    }
                }
            }
            // Inherited methods are member-scope aliases to the supertrait
            // defs, so impl completeness and static dispatch see one table.
            for parent in st.defs.get(def).sig.supertypes.clone() {
                let Some(pms) = st.defs.get(parent).member_scope else {
                    continue;
                };
                for (name, method) in st.scopes.entries(pms) {
                    if st.scopes.lookup_local(ms, name).is_none()
                        && matches!(st.defs.get(method).kind, DefKind::Fun)
                    {
                        st.scopes.insert(ms, name, method);
                    }
                }
            }
        }
        DeclKind::Aspect(a) => {
            let _ = a;
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                decl.span,
                "aspects are not supported in semantic v1",
            ));
        }
        DeclKind::Provide(p) => {
            let _ = p;
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                decl.span,
                "dependency injection is not supported in semantic v1",
            ));
        }
        DeclKind::Macro(m) => {
            // P0 compiler trust：宏展开前置 pass 未实现——此前声明被静默
            // 接受、调用点才报错；声明点稳定拒收。
            let _ = m;
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                decl.span,
                "macro declarations are not supported in v1",
            ));
        }
        // M9 9d：impl 注册延后到签名 lower 后（run() 末尾 register_impl）——
        // impl 头实例化泛型类型须先有代入前的 sig（variants/params）
        DeclKind::Impl(_) => {}
        DeclKind::Actor(a) => {
            let _ = a;
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                decl.span,
                "actors are not supported in semantic v1",
            ));
        }
        DeclKind::Comptime(_) => {
            // P0 compiler trust：comptime 无编译期求值实现，声明点稳定拒收
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                decl.span,
                "comptime declarations are not supported in v1",
            ));
        }
        DeclKind::Error(_) | DeclKind::Missing(_) => {}
    }
}

/// 顶层 / 语句 binding：模式必须是绑定形（binding/wildcard/destructure 内层绑定）
fn declare_binding_name(b: &BindingDecl, scope: ScopeId, span: Span, st: &mut SemState) {
    // M4 slice 3：`weak var` 弱引用（R4 拼写；修饰词表 weak）
    let weak = b
        .modifiers
        .modifiers
        .iter()
        .any(|m| m.word == ast::token::ModifierWord::Weak);
    if weak && b.immutable {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemUnsupportedConstruct,
            span,
            "`weak` requires a `var` binding",
        ));
    }
    if weak && b.init.is_none() {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemUnsupportedConstruct,
            span,
            "`weak` bindings require an initializer in this slice",
        ));
    }
    use ast::syntax::pat::PatKind;
    fn walk(
        pat: &ast::syntax::pat::PatWrap,
        immutable: bool,
        weak: bool,
        scope: ScopeId,
        span: Span,
        st: &mut SemState,
    ) {
        match &pat.kind {
            PatKind::Binding { name, .. } => {
                let def = add_def(
                    st,
                    scope,
                    *name,
                    span,
                    DefKind::LocalBinding {
                        mutable: !immutable,
                        weak,
                    },
                );
                // 与局部绑定同契约：check 经 pat_binding_def 精化解构元素类型
                st.pat_binding_def.set(pat.id, def);
            }
            PatKind::Or(ps)
            | PatKind::Tuple { elements: ps, .. }
            | PatKind::Array { elements: ps, .. } => {
                for p in ps {
                    walk(p, immutable, weak, scope, span, st);
                }
            }
            _ => {}
        }
    }
    let immutable = b.immutable;
    walk(&b.pattern, immutable, weak, scope, span, st);
}

// ==================== 顶层签名 lower ====================

fn lower_top_level_sigs(ast: &SourceFile, st: &mut SemState) {
    let root = st.scopes.root();
    for (i, item) in ast.items.iter().enumerate() {
        // M5 slice 2a: def via decl_def side-chain (name lookup from root
        // misses decls living in package scopes); dsc = declaring package scope
        let item_def = st.decl_def.get(i).copied().flatten();
        match &item.kind {
            DeclKind::Fun(f) => {
                let Some(def) = item_def else { continue };
                let dsc = st.defs.get(def).scope;
                let (params, ret) = if f.type_params.is_empty() {
                    (
                        lower_params(st, dsc, &f.params, item.span),
                        match &f.ret {
                            Some(t) => lower_ty(st, dsc, t),
                            None => st.tys.unit,
                        },
                    )
                } else {
                    // M7：泛型签名 lower 进类型形参作用域——T 落成
                    // TyKind::Param(index)，调用点经 TyArena::subst 代入
                    let tscope = st.defs.get(def).member_scope.unwrap_or(root);
                    (
                        lower_params(st, tscope, &f.params, item.span),
                        match &f.ret {
                            Some(t) => lower_ty(st, tscope, t),
                            None => st.tys.unit,
                        },
                    )
                };
                let defaults: Vec<bool> = f.params.iter().map(|p| p.default.is_some()).collect();
                let sig = &mut st.defs.get_mut(def).sig;
                sig.params = params;
                sig.ret = ret;
                sig.param_defaults = defaults;
            }
            DeclKind::Struct(s) => {
                let Some(def) = item_def else { continue };
                let dsc = st.defs.get(def).scope;
                // M7：泛型 struct 的字段类型 lower 进类型形参作用域（T → Param）
                let sscope = if s.type_params.is_empty() {
                    root
                } else {
                    type_param_scope(st, dsc, &s.type_params)
                };
                // M7 slice 2：类型形参默认值 lower 进签名（同 fun 纪律）
                let tp_defaults: Vec<Option<TyId>> = s
                    .type_params
                    .iter()
                    .map(|tp| tp.default.as_ref().map(|d| lower_ty(st, sscope, d)))
                    .collect();
                st.defs.get_mut(def).sig.tp_defaults = tp_defaults;
                // M9 9c：struct 形参 bounds（sscope 内解析）
                st.in_impl_header = true; // bounds 命名 trait 合法
                let tp_bounds: Vec<Vec<DefId>> = s
                    .type_params
                    .iter()
                    .map(|tp| {
                        tp.bounds
                            .iter()
                            .filter_map(|b| {
                                let bt = lower_ty(st, sscope, b);
                                match st.tys.kind(bt) {
                                    TyKind::Named(d, _)
                                        if matches!(st.defs.get(*d).kind, DefKind::Trait) =>
                                    {
                                        Some(*d)
                                    }
                                    _ => {
                                        st.diags.emit(Diagnostic::error(
                                            DiagCode::SemTypeMismatch,
                                            b.span,
                                            "type parameter bounds must name traits",
                                        ));
                                        None
                                    }
                                }
                            })
                            .collect()
                    })
                    .collect();
                st.in_impl_header = false;
                st.defs.get_mut(def).sig.tp_bounds = tp_bounds;
                // M3 主构造：头 val/var 参数类型（引用字段拒绝同纪律）+ body 字段
                let mut fields: Vec<(Symbol, TyId)> = Vec::new();
                for p in &s.primary_ctor {
                    if matches!(
                        p.modifier,
                        ast::syntax::decl::ParamModifier::Val
                            | ast::syntax::decl::ParamModifier::Var
                    ) {
                        let ty = match &p.ty {
                            Some(t) => lower_ty(st, sscope, t),
                            None => {
                                st.diags.emit(Diagnostic::error(
                                    DiagCode::SemParamTypeRequired,
                                    item.span,
                                    format!(
                                        "parameter `{}` requires a type annotation",
                                        st.interner.resolve(p.name)
                                    ),
                                ));
                                st.tys.error
                            }
                        };
                        if matches!(
                            st.tys.kind(ty),
                            TyKind::Fn(..) | TyKind::Ref { .. } | TyKind::Dyn(..)
                        ) {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemUnsupportedConstruct,
                                item.span,
                                "reference struct fields are not supported in semantic v1",
                            ));
                        }
                        fields.push((p.name, ty));
                    }
                }
                fields.extend(s.fields.iter().map(|f| {
                    let ft = lower_ty(st, sscope, &f.ty);
                    // M2 非逃逸纪律不变：引用字段拒绝（head 参数同规则）
                    if matches!(
                        st.tys.kind(ft),
                        TyKind::Fn(..) | TyKind::Ref { .. } | TyKind::Dyn(..)
                    ) {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            f.ty.span,
                            "reference struct fields are not supported in semantic v1",
                        ));
                    }
                    (f.name, ft)
                }));
                let _legacy_unused: Vec<(Symbol, TyId)> = Vec::new();
                let _ = &s.fields;
                st.defs.get_mut(def).sig.fields = fields;
                // 头构造参数（全量，含仅构造参数）+ 默认值标记进 sig.params
                if !s.primary_ctor.is_empty() {
                    let params = lower_params(st, sscope, &s.primary_ctor, item.span);
                    let defaults: Vec<bool> =
                        s.primary_ctor.iter().map(|p| p.default.is_some()).collect();
                    let sig = &mut st.defs.get_mut(def).sig;
                    sig.params = params;
                    sig.param_defaults = defaults;
                }
            }
            DeclKind::Enum(e) => {
                let Some(def) = item_def else { continue };
                // M7 slice 2：泛型 enum 变体实参 lower 进类型形参作用域
                let tscope = if e.type_params.is_empty() {
                    st.defs.get(def).scope
                } else {
                    type_param_scope(st, st.defs.get(def).scope, &e.type_params)
                };
                let dsc = tscope;
                // 形参名保留：命名实参构造（Shape.Circle(radius = 2.0)）依赖它
                let variants: Vec<(Symbol, Vec<(Symbol, TyId)>)> = e
                    .variants
                    .iter()
                    .map(|v| {
                        let args = v
                            .args
                            .iter()
                            .map(|a| (a.name, lower_variant_arg(st, dsc, a, item.span)))
                            .collect();
                        (v.name, args)
                    })
                    .collect();
                // WP1b: enum payload ARC is handled by tag-aware copy/drop traversal.
                st.defs.get_mut(def).sig.variants = variants;
            }
            DeclKind::TypeAlias(t) => {
                // Generic aliases store a Param-bearing target for use-site subst.
                if let Some(def) = item_def {
                    if t.type_params.is_empty() {
                        let dsc = st.defs.get(def).scope;
                        alias_target(st, dsc, def, &[], item.span);
                    } else {
                        let dsc = st.defs.get(def).scope;
                        let tscope = type_param_scope(st, dsc, &t.type_params);
                        st.defs.get_mut(def).sig.ty = lower_ty(st, tscope, &t.ty);
                    }
                }
            }
            DeclKind::Binding(b) => {
                if let Some(t) = &b.ty {
                    // 顶层绑定的 def 经 pat_binding_def 侧链取（declare_binding_name
                    // 不填 decl_def 槽）；标注类型 lower 进所属包作用域
                    let bdef = st.pat_binding_def.get(b.pattern.id).copied().or(item_def);
                    let dsc = bdef.map(|d| st.defs.get(d).scope).unwrap_or(root);
                    let ty = lower_ty(st, dsc, t);
                    if let ast::syntax::pat::PatKind::Binding { .. } = &b.pattern.kind
                        && let Some(def) = bdef
                    {
                        st.defs.get_mut(def).sig.ty = ty;
                    }
                }
            }
            DeclKind::Class(c) => {
                // M4 slice 1：头字段签名 lower（payload 槽全 i64——i128/聚合
                // 字段与默认值在 slice 1 拒收，保字段寻址的 8B 槽模型）
                let Some(class_def) = item_def else { continue };
                let dsc = st.defs.get(class_def).scope;
                // Phase 3a: resolve the single class supertype only after every
                // nominal declaration exists; cycles and final bases are
                // rejected at declaration phase, not left to codegen.
                if let Some(super_item) = c.supertypes.first() {
                    st.in_impl_header = true;
                    let base_ty = lower_ty(st, dsc, &super_item.ty);
                    st.in_impl_header = false;
                    let TyKind::Named(base_def, _) = st.tys.kind(base_ty) else {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemTypeMismatch,
                            super_item.ty.span,
                            "class supertype must name a class",
                        ));
                        continue;
                    };
                    let base_def = *base_def;
                    if !matches!(st.defs.get(base_def).kind, DefKind::Class) {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemTypeMismatch,
                            super_item.ty.span,
                            "class supertype must name a class",
                        ));
                        continue;
                    }
                    if st.defs.get(base_def).sig.flags.sealed
                        && !same_pkg(st, st.defs.get(base_def).scope, dsc)
                    {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            super_item.ty.span,
                            "sealed class inheritance must remain in the same package",
                        ));
                    }
                    if !st.defs.get(base_def).sig.flags.open {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            super_item.ty.span,
                            "cannot inherit a final class; mark the base `open`",
                        ));
                    }
                    if super_item.args.is_none() {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            super_item.ty.span,
                            "class inheritance requires base constructor arguments",
                        ));
                    }
                    let mut chain = vec![class_def];
                    let mut cursor = base_def;
                    loop {
                        if chain.contains(&cursor) {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemTypeCycle,
                                super_item.ty.span,
                                "class inheritance cycle",
                            ));
                            break;
                        }
                        chain.push(cursor);
                        let Some(next) = st.defs.get(cursor).sig.supertypes.first().copied() else {
                            break;
                        };
                        cursor = next;
                    }
                    st.defs.get_mut(class_def).sig.supertypes = vec![base_def];
                }
                let mut fields: Vec<(Symbol, TyId)> = Vec::new();
                if let Some(pc) = &c.primary_ctor {
                    for p in &pc.params {
                        if p.default.is_some() {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemUnsupportedConstruct,
                                item.span,
                                "class primary-constructor defaults are not supported in this slice",
                            ));
                        }
                        if !matches!(
                            p.modifier,
                            ast::syntax::decl::ParamModifier::Val
                                | ast::syntax::decl::ParamModifier::Var
                        ) {
                            continue;
                        }
                        let Some(ann) = &p.ty else {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemParamTypeRequired,
                                item.span,
                                "class constructor field requires a type annotation",
                            ));
                            continue;
                        };
                        let ty = lower_ty(st, dsc, ann);
                        if !class_field_ok(&st.tys, ty) {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemUnsupportedConstruct,
                                item.span,
                                "class fields must be 8-byte scalars in this slice (no i128/aggregates)",
                            ));
                            continue;
                        }
                        fields.push((p.name, ty));
                    }
                }
                st.defs.get_mut(class_def).sig.fields = fields;
                // 头构造参数（全量，含仅构造参数）进 sig.params——class_ctor
                // 的槽机与 sem 的实参核查都读它
                if let Some(pc) = &c.primary_ctor {
                    let params = lower_params(st, dsc, &pc.params, item.span);
                    let defaults: Vec<bool> =
                        pc.params.iter().map(|p| p.default.is_some()).collect();
                    let sig = &mut st.defs.get_mut(class_def).sig;
                    sig.params = params;
                    sig.param_defaults = defaults;
                }
                // 成员 fun 签名 lower（成员作用域已由 declare 建好）
                let ms = st.defs.get(class_def).member_scope;
                let Some(ms) = ms else { continue };
                for member in &c.members {
                    if let ClassMember::Declaration(d) = member
                        && let DeclKind::Fun(f) = &d.kind
                        && let Some(fd) = st.scopes.lookup_local(ms, f.name)
                    {
                        let params = lower_params(st, dsc, &f.params, d.span);
                        let ret = match &f.ret {
                            Some(t) => lower_ty(st, dsc, t),
                            None => st.tys.unit,
                        };
                        let defaults: Vec<bool> =
                            f.params.iter().map(|p| p.default.is_some()).collect();
                        let flags = decl_flags(&f.modifiers.modifiers);
                        let base_method = st
                            .defs
                            .get(class_def)
                            .sig
                            .supertypes
                            .first()
                            .copied()
                            .and_then(|base| member_lookup(st, base, f.name));
                        if flags.overrides {
                            let Some(base_fd) = base_method else {
                                st.diags.emit(Diagnostic::error(
                                    DiagCode::SemUnknownMember,
                                    d.span,
                                    format!(
                                        "override `{}` has no inherited method",
                                        st.interner.resolve(f.name)
                                    ),
                                ));
                                return;
                            };
                            if base_fd == fd {
                                st.diags.emit(Diagnostic::error(
                                    DiagCode::SemDuplicateDef,
                                    d.span,
                                    "method overrides itself",
                                ));
                                return;
                            }
                            let base_sig = &st.defs.get(base_fd).sig;
                            let same_shape = base_sig.params.len() == params.len()
                                && base_sig
                                    .params
                                    .iter()
                                    .zip(params.iter())
                                    .all(|((_, a), (_, b))| a == b)
                                && base_sig.ret == ret;
                            if !same_shape {
                                st.diags.emit(Diagnostic::error(
                                    DiagCode::SemTypeMismatch,
                                    d.span,
                                    format!(
                                        "override `{}` does not match the inherited signature",
                                        st.interner.resolve(f.name)
                                    ),
                                ));
                            }
                            if !base_sig.flags.open {
                                st.diags.emit(Diagnostic::error(
                                    DiagCode::SemUnsupportedConstruct,
                                    d.span,
                                    format!(
                                        "cannot override non-open method `{}`",
                                        st.interner.resolve(f.name)
                                    ),
                                ));
                            }
                        } else if flags.abstract_ {
                            if f.body.is_some() {
                                st.diags.emit(Diagnostic::error(
                                    DiagCode::SemUnsupportedConstruct,
                                    d.span,
                                    "abstract method cannot have a body",
                                ));
                            }
                            if !st.defs.get(class_def).sig.flags.abstract_ {
                                st.diags.emit(Diagnostic::error(
                                    DiagCode::SemUnsupportedConstruct,
                                    d.span,
                                    "abstract method requires an abstract class",
                                ));
                            }
                        } else if base_method.is_some() {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemDuplicateDef,
                                d.span,
                                format!(
                                    "method `{}` hides an inherited method; write `override`",
                                    st.interner.resolve(f.name)
                                ),
                            ));
                        }
                        let sig = &mut st.defs.get_mut(fd).sig;
                        sig.params = params;
                        sig.ret = ret;
                        sig.param_defaults = defaults;
                        sig.flags = flags;
                    }
                }
            }
            DeclKind::Extern(e) => {
                let Some(ed) = item_def else { continue };
                let dsc = st.defs.get(ed).scope;
                let sigs: Vec<&FunSignature> = match &e.body {
                    ExternBody::Fun(s) => vec![s],
                    ExternBody::Block { members } => members.iter().collect(),
                };
                for sig in sigs {
                    // Fun 体的 def = extern 声明自身（注册在包作用域）；
                    // Block 成员注册在同一作用域下（declare 逐名 add_def）
                    if let Some(def) = st.scopes.lookup_local(dsc, sig.name) {
                        let params = lower_params(st, dsc, &sig.params, item.span);
                        let ret = match &sig.ret {
                            Some(t) => lower_ty(st, dsc, t),
                            None => st.tys.unit,
                        };
                        // extern "c" 走真实 C ABI：i128 标量形参/返回值在本
                        // 目标上与 IR i128 传参不一致（llvme 对 runtime 入口
                        // 拆 hi/lo 规避；extern 无法替用户改 C 原型）→ 声明点
                        // 拒收。别名经 lower 归一为 Int(I128)，同样命中。
                        // 聚合参数按指针传递，成员含 i128 不受影响。
                        let is_i128_scalar =
                            |ty| matches!(st.tys.kind(ty), TyKind::Int(IntKind::I128));
                        if is_i128_scalar(ret) || params.iter().any(|(_, t)| is_i128_scalar(*t)) {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemUnsupportedConstruct,
                                item.span,
                                "extern \"c\" functions cannot take or return i128 (C ABI mismatch on this target); use i64, or pass an aggregate containing i128",
                            ));
                        }
                        let d = &mut st.defs.get_mut(def).sig;
                        d.params = params;
                        d.ret = ret;
                    }
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn lower_params(
    st: &mut SemState,
    scope: ScopeId,
    params: &[Param],
    fallback_span: Span,
) -> Vec<(Symbol, TyId)> {
    params
        .iter()
        .map(|p| {
            let ty = match &p.ty {
                Some(t) => lower_ty(st, scope, t),
                None => {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemParamTypeRequired,
                        fallback_span,
                        format!(
                            "parameter `{}` requires a type annotation",
                            st.interner.resolve(p.name)
                        ),
                    ));
                    st.tys.error
                }
            };
            (p.name, ty)
        })
        .collect()
}

fn simple_builtin_kind(name: &str) -> Option<TyKind> {
    match name {
        "i4" => Some(TyKind::Int(IntKind::I4)),
        "i8" => Some(TyKind::Int(IntKind::I8)),
        "i16" => Some(TyKind::Int(IntKind::I16)),
        "i32" => Some(TyKind::Int(IntKind::I32)),
        "i64" => Some(TyKind::Int(IntKind::I64)),
        "i128" => Some(TyKind::Int(IntKind::I128)),
        "int" => Some(TyKind::Int(IntKind::I64)),
        "f32" => Some(TyKind::Float(FloatKind::F32)),
        "f64" => Some(TyKind::Float(FloatKind::F64)),
        "bool" => Some(TyKind::Bool),
        "string" => Some(TyKind::String),
        "char" => Some(TyKind::Char),
        "unit" => Some(TyKind::Unit),
        _ => None,
    }
}

/// 枚举变体实参 lower：`Some(int)` 的裸名槽即类型名；`Some(x: int)` 走标注路径
fn lower_variant_arg(st: &mut SemState, scope: ScopeId, p: &Param, span: Span) -> TyId {
    if let Some(t) = &p.ty {
        return lower_ty(st, scope, t);
    }
    let name = p.name;
    if let Some(k) = simple_builtin_kind(st.interner.resolve(name)) {
        return st.tys.intern(k);
    }
    match lookup_with_imports(st, scope, name) {
        Some(def) => match st.defs.get(def).kind.clone() {
            DefKind::BuiltinType | DefKind::Struct | DefKind::Enum | DefKind::Class => {
                st.tys.builtin_named(def)
            }
            // M7 slice 2：变体实参的裸类型名 = 类型形参（落 Param 标记，
            // 实例化时经 subst 兑现）
            DefKind::TypeParam { index } => st.tys.intern(TyKind::Param(index)),
            DefKind::TypeAlias => alias_target(st, scope, def, &[], span),
            _ => {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUndefinedType,
                    span,
                    "variant argument is not a type",
                ));
                st.tys.error
            }
        },
        None => {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUndefinedType,
                span,
                "undefined type in variant argument",
            ));
            st.tys.error
        }
    }
}
// ==================== 类型 lower（Named 路径解析 + 别名展开） ====================

pub(crate) fn lower_ty(st: &mut SemState, scope: ScopeId, ty: &TypeWrap) -> TyId {
    if let Some(&cached) = st.ty_of_node.get(ty.id) {
        return cached;
    }
    let out = lower_ty_uncached(st, scope, ty);
    st.ty_of_node.set(ty.id, out);
    out
}

fn lower_ty_uncached(st: &mut SemState, scope: ScopeId, ty: &TypeWrap) -> TyId {
    match &ty.kind {
        TypeKind::Unit => st.tys.unit,
        TypeKind::Named { path, type_args } => {
            if path.segments.is_empty() {
                return st.tys.error;
            }
            if path.segments.len() == 1
                && let Some(k) = simple_builtin_kind(st.interner.resolve(path.segments[0].name))
            {
                return st.tys.intern(k);
            }
            let Some(def) = resolve_path(st, scope, path) else {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUndefinedType,
                    ty.span,
                    "undefined type",
                ));
                return st.tys.error;
            };
            let args: Vec<TyId> = type_args
                .iter()
                .map(|a| lower_ty(st, scope, &a.ty))
                .collect();
            // P0 compiler trust：trait 只允许出现在 lower 的根位置（bounds、
            // impl trait 槽经 in_impl_header 放行）；嵌进类型实参一律拒收
            //（`impl T for L<T>` 此前静默注册成 spec、调用点误报）。
            if args.iter().any(|a| {
                matches!(
                    st.tys.kind(*a),
                    TyKind::Named(d, _) if matches!(st.defs.get(*d).kind, DefKind::Trait)
                )
            }) {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnsupportedConstruct,
                    ty.span,
                    "trait cannot be used as a type argument",
                ));
                return st.tys.error;
            }
            let kind = st.defs.get(def).kind.clone();
            match kind {
                // M7：类型形参——克隆体经 mono_subst 兑现具体类型；
                // 泛型原件（未检查体）落成 TyKind::Param(index)
                DefKind::TypeParam { index } => st
                    .mono_subst
                    .iter()
                    .find(|(d, _)| *d == def)
                    .map(|(_, t)| *t)
                    .unwrap_or_else(|| st.tys.intern(TyKind::Param(index))),
                DefKind::BuiltinType => st.tys.intern(TyKind::Named(def, args)),
                // M9 9a：trait 不作类型标注（Q6 静态分派 only；
                // impl 头解析期经 in_impl_header 放行）
                DefKind::Trait if !st.in_impl_header => {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemUnsupportedConstruct,
                        ty.span,
                        "traits cannot be used as types (static dispatch only)",
                    ));
                    st.tys.error
                }
                DefKind::Trait => st.tys.intern(TyKind::Named(def, args)),
                DefKind::Struct => {
                    let want = st.defs.get(def).ty_params as usize;
                    if want == 0 {
                        if !args.is_empty() {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemTypeArgMismatch,
                                ty.span,
                                "this struct is not generic",
                            ));
                        }
                        st.tys.intern(TyKind::Named(def, args))
                    } else if args.len() != want {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemTypeArgMismatch,
                            ty.span,
                            format!(
                                "generic struct expects {want} type argument(s), got {}",
                                args.len()
                            ),
                        ));
                        st.tys.error
                    } else if args.iter().any(|a| st.tys.mentions_param(*a)) {
                        // M7：泛型签名 lower 期（`struct A<T> { b: B<T> }` 的
                        // 字段）——形参未代入，保留 Named(原件, [Param]) 占位；
                        // 调用点实例化时经 subst+concretize 落 spec def
                        st.tys.intern(TyKind::Named(def, args))
                    } else {
                        // M7：实例化 = spec def（代入后的 fields/params），
                        // 值类型一律 Named(spec, [])——下游布局/成员解析零改动
                        let spec = crate::mono::instantiate_struct(st, def, &args);
                        st.tys.builtin_named(spec)
                    }
                }
                // M7 slice 2：泛型 enum 实例化 = spec def（变体代入后）
                DefKind::Enum => {
                    let want = st.defs.get(def).ty_params as usize;
                    if want > 0 {
                        if args.len() != want {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemTypeArgMismatch,
                                ty.span,
                                format!(
                                    "generic enum expects {want} type argument(s), got {}",
                                    args.len()
                                ),
                            ));
                            return st.tys.error;
                        }
                        let spec = crate::mono::instantiate_enum(st, def, &args);
                        st.tys.builtin_named(spec)
                    } else {
                        if !args.is_empty() {
                            st.diags.emit(Diagnostic::error(
                                DiagCode::SemTypeArgMismatch,
                                ty.span,
                                "this enum is not generic",
                            ));
                        }
                        st.tys.intern(TyKind::Named(def, args))
                    }
                }
                DefKind::Class => {
                    if !args.is_empty() {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            ty.span,
                            "generic instantiation is not supported in semantic v1",
                        ));
                    }
                    st.tys.intern(TyKind::Named(def, args))
                }
                DefKind::TypeAlias => {
                    let want = st.defs.get(def).ty_params as usize;
                    if args.len() != want {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemTypeArgMismatch,
                            ty.span,
                            format!(
                                "type alias expects {want} type argument(s), got {}",
                                args.len()
                            ),
                        ));
                        st.tys.error
                    } else {
                        alias_target(st, scope, def, &args, ty.span)
                    }
                }
                _ => {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemUnsupportedConstruct,
                        ty.span,
                        "this name is not a type",
                    ));
                    st.tys.error
                }
            }
        }
        TypeKind::Ref { mutable, inner } => {
            let i = lower_ty(st, scope, inner);
            st.tys.intern(TyKind::Ref {
                mutable: *mutable,
                inner: i,
            })
        }
        TypeKind::Pointer { mutable, inner } => {
            let i = lower_ty(st, scope, inner);
            st.tys.intern(TyKind::Pointer {
                mutable: *mutable,
                inner: i,
            })
        }
        TypeKind::Nullable(inner) => {
            let t = lower_ty(st, scope, inner);
            // M4 slice 4a：可空 class（?P）放行——值模型 = 2 槽
            //（i8 flag + ptr payload），布局/槽类型见 irc::layout 的 class 臂
            st.tys.nullable(t)
        }
        TypeKind::Array { element, len } => {
            let t = lower_ty(st, scope, element);
            st.tys.intern(TyKind::Array(t, *len))
        }
        TypeKind::Tuple(ts) => {
            let mut out = Vec::with_capacity(ts.len());
            for t in ts {
                out.push(lower_ty(st, scope, t));
            }
            st.tys.intern(TyKind::Tuple(out))
        }
        TypeKind::Fn { params, ret } => {
            let mut ps = Vec::with_capacity(params.len());
            for p in params {
                ps.push(lower_ty(st, scope, p));
            }
            let r = lower_ty(st, scope, ret);
            st.tys.intern(TyKind::Fn(ps, r))
        }
        // M9 9a：Self = 当前 trait 声明的作用域内占位类型（impl 核对时
        // 代入具体类型）；trait 之外使用 = 诊断
        TypeKind::SelfType => {
            let self_sym = st.interner.intern("Self");
            match lookup_with_imports(st, scope, self_sym) {
                Some(d) if matches!(st.defs.get(d).kind, DefKind::BuiltinType) => {
                    st.tys.intern(TyKind::Named(d, vec![]))
                }
                _ => {
                    st.diags.emit(Diagnostic::error(
                        DiagCode::SemUndefinedType,
                        ty.span,
                        "`Self` is only available inside trait declarations",
                    ));
                    st.tys.error
                }
            }
        }
        TypeKind::Dyn(inner) => {
            // The dyn wrapper legitimizes a trait root, unlike normal type
            // annotation lowering. Restore the caller's impl-header state.
            let saved_impl_header = st.in_impl_header;
            st.in_impl_header = true;
            let inner = lower_ty(st, scope, inner);
            st.in_impl_header = saved_impl_header;
            let Some(trait_def) = (match st.tys.kind(inner) {
                TyKind::Named(d, _) if matches!(st.defs.get(*d).kind, DefKind::Trait) => Some(*d),
                _ => None,
            }) else {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemTypeMismatch,
                    ty.span,
                    "dyn target must name a trait",
                ));
                return st.tys.error;
            };
            let TyKind::Named(_, args) = st.tys.kind(inner) else {
                unreachable!("trait target was matched as Named above")
            };
            if !args.is_empty() {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnsupportedConstruct,
                    ty.span,
                    "generic dyn is not supported in this slice",
                ));
                return st.tys.error;
            }
            st.tys.intern(TyKind::Dyn(trait_def, Vec::new()))
        }
        TypeKind::Union(_) => {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                ty.span,
                "union types are not supported in semantic v1",
            ));
            st.tys.error
        }
        TypeKind::Error(_) | TypeKind::Missing(_) => st.tys.error,
    }
}

fn alias_target(st: &mut SemState, scope: ScopeId, def: DefId, args: &[TyId], span: Span) -> TyId {
    if st.defs.get(def).ty_params > 0 {
        let target = st.defs.get(def).sig.ty;
        if st.tys.is_error(target) {
            return target;
        }
        let substituted = st.tys.subst(target, args);
        return crate::mono::concretize(st, substituted);
    }
    if st.defs.get(def).resolving {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemTypeCycle,
            span,
            "type alias cycle",
        ));
        return st.tys.error;
    }
    let Some(alias_of) = st.defs.get(def).alias_of.clone() else {
        return st.tys.error;
    };
    st.defs.get_mut(def).resolving = true;
    let ty = lower_ty(st, scope, &alias_of);
    st.defs.get_mut(def).resolving = false;
    st.defs.get_mut(def).sig.ty = ty;
    // 目标为命名类型时预解析（值位置的别名经此走到构造器分支）
    if let TyKind::Named(target, _) = st.tys.kind(ty).clone() {
        st.defs.get_mut(def).alias_resolved = Some(target);
    }
    ty
}

// ==================== 路径解析（import 展开 + 成员查找） ====================

pub(crate) fn resolve_path(
    st: &SemState,
    scope: ScopeId,
    path: &ast::syntax::node::Path,
) -> Option<DefId> {
    let first = path.segments.first()?;
    let mut cur = lookup_with_imports(st, scope, first.name)?;
    for seg in &path.segments[1..] {
        cur = member_lookup(st, cur, seg.name)?;
    }
    Some(cur)
}

pub(crate) fn lookup_with_imports(st: &SemState, scope: ScopeId, name: Symbol) -> Option<DefId> {
    let mut cur = Some(scope);
    while let Some(s) = cur {
        if let Some(def) = st.scopes.lookup_local(s, name) {
            return expand_import(st, def);
        }
        cur = st.scopes.parent_of(s);
    }
    None
}

fn expand_import(st: &SemState, def: DefId) -> Option<DefId> {
    let segments = match &st.defs.get(def).kind {
        DefKind::ImportNs { segments } => segments.clone(),
        _ => return Some(def),
    };
    let root = st.scopes.root();
    let mut cur = lookup_local_no_import(st, root, *segments.first()?)?;
    for seg in &segments[1..] {
        cur = member_lookup(st, cur, *seg)?;
    }
    Some(cur)
}

fn lookup_local_no_import(st: &SemState, scope: ScopeId, name: Symbol) -> Option<DefId> {
    let mut cur = Some(scope);
    while let Some(s) = cur {
        if let Some(def) = st.scopes.lookup_local(s, name) {
            if matches!(st.defs.get(def).kind, DefKind::ImportNs { .. }) {
                return expand_import(st, def);
            }
            return Some(def);
        }
        cur = st.scopes.parent_of(s);
    }
    None
}

pub(crate) fn member_lookup(st: &SemState, def: DefId, name: Symbol) -> Option<DefId> {
    // 别名归一：`type F = E` 的 F.A 按 E.A 解析（值位置已由 def_value_ty 归一）
    let def = st.defs.get(def).alias_resolved.unwrap_or(def);
    let ms = st.defs.get(def).member_scope?;
    if let Some(found) = st.scopes.lookup_local(ms, name) {
        return expand_import(st, found);
    }
    // D-019: inherited class members resolve along the single base chain.
    let base = st.defs.get(def).sig.supertypes.first().copied()?;
    if !matches!(st.defs.get(base).kind, DefKind::Class) {
        return None;
    }
    member_lookup(st, base, name)
}
