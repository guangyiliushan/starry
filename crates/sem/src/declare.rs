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
use crate::ty::{FloatKind, IntKind, TyArena, TyId, TyKind};

const BUILTINS: &[&str] = &[
    "i4", "i8", "i16", "i32", "i64", "i128", "int", "f32", "f64", "bool", "string", "char", "unit",
    "Own", "Shared",
];

pub(crate) fn run(ast: &SourceFile, st: &mut SemState) {
    register_builtins(st);
    let root = st.scopes.root();
    // M5 slice 2a：按序跟踪当前包段；顶层声明进包作用域（无段 = 根）
    let mut cur = root;
    let mut pkgs: std::collections::HashMap<String, ScopeId> = std::collections::HashMap::new();
    // M5 slice 2b：import 延后到全部真实声明之后注册——同包本地定义
    // 恒优先于导入别名（同包自导入、跨文件同名合法化）；导入互撞仍诊断
    let mut pending_imports: Vec<(ScopeId, &DeclNode)> = Vec::new();
    for (i, item) in ast.items.iter().enumerate() {
        if let DeclKind::PackageSeg(path) = &item.kind {
            cur = pkg_scope(path, root, &mut pkgs, st);
            st.decl_def.push(None); // 保持 decl_def 与 items 1:1
            continue;
        }
        if matches!(item.kind, DeclKind::Import(_)) {
            st.decl_def.push(None);
            pending_imports.push((cur, item));
            continue;
        }
        declare_one(item, i, cur, st);
    }
    for (scope, item) in pending_imports {
        register_import(scope, item, st);
    }
    lower_top_level_sigs(ast, st);
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
        if let Some(old) = st.scopes.lookup_local(scope, name)
            && !matches!(st.defs.get(old).kind, DefKind::ImportNs { .. })
        {
            return; // 本地定义优先于导入
        }
        add_def(st, scope, name, decl.span, DefKind::ImportNs { segments });
    }
    // Native import：v1 注册别名名（调用解析为 unsupported）
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
        declare_one(item, i, root, st);
    }
}

/// 逐项声明：先占 decl_def 槽（与 items 1:1 对齐），再登记名字
fn declare_one(decl: &DeclNode, index: usize, scope: ScopeId, st: &mut SemState) {
    st.decl_def.push(None);
    declare_decl(decl, index, scope, st);
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
    where_clause: &[WhereConstraint],
    span: Span,
    st: &mut SemState,
    what: &str,
) {
    let fancy = type_params
        .iter()
        .any(|tp| tp.variance.is_some() || !tp.bounds.is_empty() || tp.default.is_some());
    if fancy {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemUnsupportedConstruct,
            span,
            format!(
                "type parameter bounds/defaults/variance on generic {what} are not supported in this slice"
            ),
        ));
    }
    if !where_clause.is_empty() {
        st.diags.emit(Diagnostic::error(
            DiagCode::SemUnsupportedConstruct,
            span,
            format!("where clauses on generic {what} are not supported in this slice"),
        ));
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
            if !e.type_params.is_empty() {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnsupportedConstruct,
                    decl.span,
                    "generic enums are not supported in this slice",
                ));
            }
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
                        st.diags.emit(Diagnostic::error(
                            DiagCode::SemUnsupportedConstruct,
                            d.span,
                            "class body field bindings are not supported in this slice (use primary-constructor val/var parameters)",
                        ));
                    }
                    ClassMember::Declaration(d) if matches!(&d.kind, DeclKind::Fun(_)) => {
                        if let DeclKind::Fun(f) = &d.kind {
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
                    _ => {}
                }
            }
        }
        DeclKind::TypeAlias(t) => {
            if !t.type_params.is_empty() {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnsupportedConstruct,
                    decl.span,
                    "generic type aliases are not supported in this slice",
                ));
            }
            let def = add_def(st, scope, t.name, decl.span, DefKind::TypeAlias);
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
                ExternBody::Block { members } | ExternBody::Namespace { members, .. } => {
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
            if !u.type_params.is_empty() {
                st.diags.emit(Diagnostic::error(
                    DiagCode::SemUnsupportedConstruct,
                    decl.span,
                    "generic unions are not supported in semantic v1",
                ));
            }
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
                        if matches!(st.tys.kind(ty), TyKind::Ref { .. }) {
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
                    if matches!(st.tys.kind(ft), TyKind::Ref { .. }) {
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
                let dsc = st.defs.get(def).scope;
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
                st.defs.get_mut(def).sig.variants = variants;
            }
            DeclKind::TypeAlias(_t) => {
                // 急切 lower：值位置（F.A / O.Some(1)）不经 lower_ty，
                // 惰性会让 alias_resolved 缺席、成员解析失败；循环守卫仍在 lower_ty
                if let Some(def) = item_def {
                    let dsc = st.defs.get(def).scope;
                    alias_target(st, dsc, def, item.span);
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
                        let sig = &mut st.defs.get_mut(fd).sig;
                        sig.params = params;
                        sig.ret = ret;
                        sig.param_defaults = defaults;
                    }
                }
            }
            DeclKind::Extern(e) => {
                let Some(ed) = item_def else { continue };
                let dsc = st.defs.get(ed).scope;
                let sigs: Vec<&FunSignature> = match &e.body {
                    ExternBody::Fun(s) => vec![s],
                    ExternBody::Block { members } | ExternBody::Namespace { members, .. } => {
                        members.iter().collect()
                    }
                };
                for sig in sigs {
                    // Fun 体的 def = extern 声明自身（注册在包作用域）；
                    // Block/Namespace 成员注册在同一作用域下（declare 逐名 add_def）
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
                // M7：类型形参——克隆体经 mono_subst 兑现具体类型；
                // 泛型原件（未检查体）落成 TyKind::Param(index)
                DefKind::TypeParam { index } => st
                    .mono_subst
                    .iter()
                    .find(|(d, _)| *d == def)
                    .map(|(_, t)| *t)
                    .unwrap_or_else(|| st.tys.intern(TyKind::Param(index))),
                DefKind::BuiltinType => st.tys.intern(TyKind::Named(def, args)),
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
                DefKind::Enum | DefKind::Class => {
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
