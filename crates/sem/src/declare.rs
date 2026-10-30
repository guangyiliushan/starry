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
use crate::def::{Def, DefId, DefKind, DefSig, ScopeId};
use crate::ty::{FloatKind, IntKind, TyId, TyKind};

const BUILTINS: &[&str] = &[
    "i4", "i8", "i16", "i32", "i64", "i128", "int", "f32", "f64", "bool", "string", "char", "unit",
    "Own", "Shared",
];

pub(crate) fn run(ast: &SourceFile, st: &mut SemState) {
    register_builtins(st);
    let root = st.scopes.root();
    for item in &ast.items {
        declare_decl(item, root, st);
    }
    lower_top_level_sigs(ast, st);
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

fn declare_decl(decl: &DeclNode, scope: ScopeId, st: &mut SemState) {
    match &decl.kind {
        DeclKind::Import(i) => {
            if let ImportKind::Qualified { path, suffix } = &i.kind {
                let segments: Vec<Symbol> = path.segments.iter().map(|s| s.name).collect();
                let name = match suffix {
                    ImportSuffix::Alias(a) => *a,
                    _ => match segments.last() {
                        Some(s) => *s,
                        None => return,
                    },
                };
                if !matches!(suffix, ImportSuffix::Star) {
                    add_def(st, scope, name, decl.span, DefKind::ImportNs { segments });
                }
            }
            // Native import：v1 注册别名名（调用解析为 unsupported）
        }
        DeclKind::Fun(f) => {
            if !f.type_params.is_empty() {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnsupportedConstruct,
                    decl.span,
                    "generic functions are not supported in semantic v1",
                ));
            }
            add_def(st, scope, f.name, decl.span, DefKind::Fun);
        }
        DeclKind::Binding(b) => {
            declare_binding_name(b, scope, decl.span, st);
        }
        DeclKind::Struct(s) => {
            let def = add_def(st, scope, s.name, decl.span, DefKind::Struct);
            let ms = st.scopes.new_scope(Some(scope));
            st.defs.get_mut(def).member_scope = Some(ms);
            for (index, field) in s.fields.iter().enumerate() {
                add_def(
                    st,
                    ms,
                    field.name,
                    decl.span,
                    DefKind::StructField { parent: def, index },
                );
            }
        }
        DeclKind::Enum(e) => {
            let def = add_def(st, scope, e.name, decl.span, DefKind::Enum);
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
            // v1 不支持 class（无构造路径/方法调用）：注册保持不级联，钉一条诊断
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                decl.span,
                "class declarations are not supported in semantic v1",
            ));
            let def = add_def(st, scope, c.name, decl.span, DefKind::Class);
            let ms = st.scopes.new_scope(Some(scope));
            st.defs.get_mut(def).member_scope = Some(ms);
            for member in &c.members {
                if let ClassMember::Declaration(d) = member
                    && let DeclKind::Fun(f) = &d.kind
                {
                    add_def(st, ms, f.name, d.span, DefKind::Fun);
                }
            }
        }
        DeclKind::TypeAlias(t) => {
            let def = add_def(st, scope, t.name, decl.span, DefKind::TypeAlias);
            st.defs.get_mut(def).alias_of = Some(t.ty.clone());
        }
        DeclKind::Extern(e) => match &e.body {
            ExternBody::Fun(sig) => {
                add_def(st, scope, sig.name, decl.span, DefKind::Fun);
            }
            ExternBody::Block { members } | ExternBody::Namespace { members, .. } => {
                for sig in members {
                    add_def(st, scope, sig.name, decl.span, DefKind::Fun);
                }
            }
        },
        DeclKind::Union(u) => {
            add_def(st, scope, u.name, decl.span, DefKind::Struct);
        }
        DeclKind::Trait(t) => {
            let _ = t;
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                decl.span,
                "traits are not supported in semantic v1",
            ));
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
            let _ = m;
            // 宏定义注册：宏体展开是独立前置 pass（规格待定）；v1 展开前调用点报错
        }
        DeclKind::Impl(i) => {
            let _ = i;
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                decl.span,
                "impl blocks are not supported in semantic v1",
            ));
        }
        DeclKind::Actor(a) => {
            let _ = a;
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                decl.span,
                "actors are not supported in semantic v1",
            ));
        }
        DeclKind::Comptime(_) => {}
        DeclKind::Error(_) | DeclKind::Missing(_) => {}
    }
}

/// 顶层 / 语句 binding：模式必须是绑定形（binding/wildcard/destructure 内层绑定）
fn declare_binding_name(b: &BindingDecl, scope: ScopeId, span: Span, st: &mut SemState) {
    use ast::syntax::pat::PatKind;
    fn walk(
        pat: &ast::syntax::pat::PatWrap,
        immutable: bool,
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
                    },
                );
                // 与局部绑定同契约：check 经 pat_binding_def 精化解构元素类型
                st.pat_binding_def.set(pat.id, def);
            }
            PatKind::Or(ps)
            | PatKind::Tuple { elements: ps, .. }
            | PatKind::Array { elements: ps, .. } => {
                for p in ps {
                    walk(p, immutable, scope, span, st);
                }
            }
            _ => {}
        }
    }
    let immutable = b.immutable;
    walk(&b.pattern, immutable, scope, span, st);
}

// ==================== 顶层签名 lower ====================

fn lower_top_level_sigs(ast: &SourceFile, st: &mut SemState) {
    let root = st.scopes.root();
    for item in &ast.items {
        match &item.kind {
            DeclKind::Fun(f) => {
                let def = match st.scopes.lookup_local(root, f.name) {
                    Some(d) => d,
                    None => continue,
                };
                // 泛型 fun 不 lower（T 未定义会级联 SemUndefinedType）；
                // 参数名保留维持 arity，类型走投毒 Error
                let (params, ret) = if f.type_params.is_empty() {
                    (
                        lower_params(st, root, &f.params, item.span),
                        match &f.ret {
                            Some(t) => lower_ty(st, root, t),
                            None => st.tys.unit,
                        },
                    )
                } else {
                    (
                        f.params.iter().map(|p| (p.name, st.tys.error)).collect(),
                        st.tys.error,
                    )
                };
                let defaults: Vec<bool> = f.params.iter().map(|p| p.default.is_some()).collect();
                let sig = &mut st.defs.get_mut(def).sig;
                sig.params = params;
                sig.ret = ret;
                sig.param_defaults = defaults;
            }
            DeclKind::Struct(s) => {
                let def = match st.scopes.lookup_local(root, s.name) {
                    Some(d) => d,
                    None => continue,
                };
                let fields: Vec<(Symbol, TyId)> = s
                    .fields
                    .iter()
                    .map(|f| (f.name, lower_ty(st, root, &f.ty)))
                    .collect();
                st.defs.get_mut(def).sig.fields = fields;
            }
            DeclKind::Enum(e) => {
                let def = match st.scopes.lookup_local(root, e.name) {
                    Some(d) => d,
                    None => continue,
                };
                // 形参名保留：命名实参构造（Shape.Circle(radius = 2.0)）依赖它
                let variants: Vec<(Symbol, Vec<(Symbol, TyId)>)> = e
                    .variants
                    .iter()
                    .map(|v| {
                        let args = v
                            .args
                            .iter()
                            .map(|a| (a.name, lower_variant_arg(st, root, a, item.span)))
                            .collect();
                        (v.name, args)
                    })
                    .collect();
                st.defs.get_mut(def).sig.variants = variants;
            }
            DeclKind::TypeAlias(t) => {
                // 急切 lower：值位置（F.A / O.Some(1)）不经 lower_ty，
                // 惰性会让 alias_resolved 缺席、成员解析失败；循环守卫仍在 lower_ty
                if let Some(def) = st.scopes.lookup_local(root, t.name) {
                    alias_target(st, root, def, item.span);
                }
            }
            DeclKind::Binding(b) => {
                if let Some(t) = &b.ty {
                    let ty = lower_ty(st, root, t);
                    if let ast::syntax::pat::PatKind::Binding { name, .. } = &b.pattern.kind
                        && let Some(def) = st.scopes.lookup_local(root, *name)
                    {
                        st.defs.get_mut(def).sig.ty = ty;
                    }
                }
            }
            DeclKind::Class(c) => {
                // 成员 fun 签名 lower（成员作用域已由 declare 建好）
                let class_def = match st.scopes.lookup_local(root, c.name) {
                    Some(d) => d,
                    None => continue,
                };
                let ms = st.defs.get(class_def).member_scope;
                let Some(ms) = ms else { continue };
                for member in &c.members {
                    if let ClassMember::Declaration(d) = member
                        && let DeclKind::Fun(f) = &d.kind
                        && let Some(fd) = st.scopes.lookup_local(ms, f.name)
                    {
                        let params = lower_params(st, root, &f.params, d.span);
                        let ret = match &f.ret {
                            Some(t) => lower_ty(st, root, t),
                            None => st.tys.unit,
                        };
                        let defaults: Vec<bool> =
                            f.params.iter().map(|p| p.default.is_some()).collect();
                        let sig = &mut st.defs.get_mut(fd).sig;
                        sig.params = params;
                        sig.ret = ret;
                        sig.param_defaults = defaults;
                    }
                }
            }
            DeclKind::Extern(e) => {
                let sigs: Vec<&FunSignature> = match &e.body {
                    ExternBody::Fun(s) => vec![s],
                    ExternBody::Block { members } | ExternBody::Namespace { members, .. } => {
                        members.iter().collect()
                    }
                };
                for sig in sigs {
                    if let Some(def) = st.scopes.lookup_local(root, sig.name) {
                        let params = lower_params(st, root, &sig.params, item.span);
                        let ret = match &sig.ret {
                            Some(t) => lower_ty(st, root, t),
                            None => st.tys.unit,
                        };
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
            DefKind::TypeAlias => alias_target(st, scope, def, span),
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
            let kind = st.defs.get(def).kind.clone();
            match kind {
                DefKind::BuiltinType => st.tys.intern(TyKind::Named(def, args)),
                DefKind::Struct | DefKind::Enum | DefKind::Class => {
                    if !args.is_empty() {
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            ty.span,
                            "generic instantiation is not supported in semantic v1",
                        ));
                    }
                    st.tys.intern(TyKind::Named(def, args))
                }
                DefKind::TypeAlias => alias_target(st, scope, def, ty.span),
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
        TypeKind::Nullable(inner) => {
            let t = lower_ty(st, scope, inner);
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
        TypeKind::Union(_) | TypeKind::Dyn(_) | TypeKind::SelfType => {
            st.diags.emit(Diagnostic::error(
                DiagCode::SemUnsupportedConstruct,
                ty.span,
                "union/dyn/Self types are not supported in semantic v1",
            ));
            st.tys.error
        }
        TypeKind::Error(_) | TypeKind::Missing(_) => st.tys.error,
    }
}

fn alias_target(st: &mut SemState, scope: ScopeId, def: DefId, span: Span) -> TyId {
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
    let found = st.scopes.lookup_local(ms, name)?;
    expand_import(st, found)
}
