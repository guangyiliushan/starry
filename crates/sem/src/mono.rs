//! M7 单态化：实例化注册表 + 波次驱动（设计见 docs/syntax/generic-mono-spike.md）。
//!
//! - fun 实例化 = 克隆声明 AST + NodeId 重编号 + 增量跑 declare/resolve/check；
//! - struct 实例化 = spec def（代入后的 sig.fields/params + 克隆字段作用域），
//!   无 AST 面，调用点同步完成；
//! - 值类型一律 `Named(spec_def, [])`：irc/opt/llvme 对实例化产物零改动。

use ast::syntax::decl::DeclKind;
use ast::syntax::ty::TypeParamName;

use crate::Sem;
use crate::SemState;
use crate::def::{Def, DefId, DefKind, DefSig};
use crate::ty::{TyId, TyKind};

/// fun 实例化作业（struct 在调用点同步完成，不排队）
pub(crate) struct MonoJob {
    pub def: DefId,
    pub args: Vec<TyId>,
}

/// type_call/lower_ty 的统一入口：取或建实例化 def。
pub(crate) fn instantiate(st: &mut SemState, def: DefId, args: &[TyId]) -> DefId {
    if let Some(&spec) = st.mono_insts.get(&(def, args.to_vec())) {
        return spec;
    }
    match st.defs.get(def).kind {
        DefKind::Struct => instantiate_struct(st, def, args),
        DefKind::Fun => {
            // spec def 立即占位（expr_def 指向它，irc 按 DefId 找体），
            // AST 克隆与签名 lower 排到波次（需要 &mut SourceFile）
            let (name, span, scope, unit, error) = {
                let d = st.defs.get(def);
                (d.name, d.span, d.scope, st.tys.unit, st.tys.error)
            };
            let spec = st.defs.push(Def {
                kind: DefKind::Fun,
                name,
                scope,
                span,
                sig: DefSig::new(unit, error),
                resolving: false,
                alias_of: None,
                alias_resolved: None,
                member_scope: None,
                external: false,
                ty_params: 0,
                decl: None,
            });
            st.mono_insts.insert((def, args.to_vec()), spec);
            st.mono_queue.push(MonoJob {
                def,
                args: args.to_vec(),
            });
            spec
        }
        _ => def,
    }
}

/// struct 实例化：spec def。字段/构造参数类型 = subst + concretize
/// （nested 泛型 `A<T>{ b: B<T> }` 代入后须把 B<int> 也落到 spec def）。
pub(crate) fn instantiate_struct(st: &mut SemState, def: DefId, args: &[TyId]) -> DefId {
    debug_assert!(
        !args.iter().any(|a| st.tys.mentions_param(*a)),
        "实例化实参必须已具体（Param 占位应在签名 lower 期拦截）"
    );
    if let Some(&spec) = st.mono_insts.get(&(def, args.to_vec())) {
        return spec;
    }
    let (name, span, scope, unit, error) = {
        let d = st.defs.get(def);
        (d.name, d.span, d.scope, st.tys.unit, st.tys.error)
    };
    let spec = st.defs.push(Def {
        kind: DefKind::Struct,
        name,
        scope,
        span,
        sig: DefSig::new(unit, error),
        resolving: false,
        alias_of: None,
        alias_resolved: None,
        member_scope: None,
        external: false,
        ty_params: 0,
        decl: None,
    });
    // 先注册再填 sig：concretize 递归实例化 nested 泛型时可能回指本 def
    st.mono_insts.insert((def, args.to_vec()), spec);
    let (fields, params, defaults) = {
        let orig = st.defs.get(def);
        let fields: Vec<_> = orig.sig.fields.iter().map(|(n, t)| (*n, *t)).collect();
        let params: Vec<_> = orig.sig.params.iter().map(|(n, t)| (*n, *t)).collect();
        (fields, params, orig.sig.param_defaults.clone())
    };
    let mut sfields = Vec::with_capacity(fields.len());
    for (n, t) in &fields {
        let ty = st.tys.subst(*t, args);
        let ty = concretize(st, ty);
        sfields.push((*n, ty));
    }
    let mut sparams = Vec::with_capacity(params.len());
    for (n, t) in &params {
        let ty = st.tys.subst(*t, args);
        let ty = concretize(st, ty);
        sparams.push((*n, ty));
    }
    {
        let sig = &mut st.defs.get_mut(spec).sig;
        sig.fields = sfields.clone();
        sig.params = sparams;
        sig.param_defaults = defaults;
    }
    // member_scope：克隆字段 def（可变性三态随字段走，parent 指 spec）
    let ms = st.scopes.new_scope(Some(scope));
    for (index, (fname, _)) in sfields.iter().enumerate() {
        let mutable = member_lookup(st, def, *fname).and_then(|fd| match st.defs.get(fd).kind {
            DefKind::StructField { mutable, .. } => mutable,
            _ => None,
        });
        let fdef = st.defs.push(Def {
            kind: DefKind::StructField {
                parent: spec,
                index,
                mutable,
            },
            name: *fname,
            scope: ms,
            span,
            sig: DefSig::new(unit, error),
            resolving: false,
            alias_of: None,
            alias_resolved: None,
            member_scope: None,
            external: false,
            ty_params: 0,
            decl: None,
        });
        st.scopes.insert(ms, *fname, fdef);
    }
    st.defs.get_mut(spec).member_scope = Some(ms);
    spec
}

/// subst 后的收尾：把仍指向泛型原件的 Named(原件, 具体args) 逐层换成
/// Named(spec, [])（布局/成员解析只认 spec def 的具体 sig）。
fn concretize(st: &mut SemState, ty: TyId) -> TyId {
    match st.tys.kind(ty).clone() {
        TyKind::Named(def, args) => {
            let args: Vec<TyId> = args.iter().map(|a| concretize(st, *a)).collect();
            if st.defs.get(def).ty_params > 0 && !args.is_empty() {
                if args.iter().any(|a| st.tys.mentions_param(*a)) {
                    // 防御：subst 输出应已具体；占位不实例化
                    return st.tys.intern(TyKind::Named(def, args));
                }
                let spec = instantiate_struct(st, def, &args);
                st.tys.builtin_named(spec)
            } else {
                st.tys.intern(TyKind::Named(def, args))
            }
        }
        TyKind::Ref { mutable, inner } => {
            let i = concretize(st, inner);
            st.tys.intern(TyKind::Ref { mutable, inner: i })
        }
        TyKind::Pointer { mutable, inner } => {
            let i = concretize(st, inner);
            st.tys.intern(TyKind::Pointer { mutable, inner: i })
        }
        TyKind::Nullable(inner) => {
            let i = concretize(st, inner);
            st.tys.nullable(i)
        }
        TyKind::Array(elem, len) => {
            let e = concretize(st, elem);
            st.tys.intern(TyKind::Array(e, len))
        }
        TyKind::Tuple(elems) => {
            let ts = elems.iter().map(|t| concretize(st, *t)).collect();
            st.tys.intern(TyKind::Tuple(ts))
        }
        TyKind::Range(inner) => {
            let i = concretize(st, inner);
            st.tys.intern(TyKind::Range(i))
        }
        TyKind::Fn(ps, ret) => {
            let params = ps.iter().map(|t| concretize(st, *t)).collect();
            let r = concretize(st, ret);
            st.tys.intern(TyKind::Fn(params, r))
        }
        _ => ty,
    }
}

/// 泛型 fun 的调用点定型：代入后的形参/返回值（nested 泛型落 spec）
pub(crate) fn subst_params(
    st: &mut SemState,
    def: DefId,
    args: &[TyId],
) -> (Vec<(ast::interner::Symbol, TyId)>, TyId, Vec<bool>) {
    let (params, ret, defaults) = {
        let sig = &st.defs.get(def).sig;
        (sig.params.clone(), sig.ret, sig.param_defaults.clone())
    };
    let mut sp = Vec::with_capacity(params.len());
    for (n, t) in &params {
        let ty = st.tys.subst(*t, args);
        let ty = concretize(st, ty);
        sp.push((*n, ty));
    }
    let sret = {
        let ty = st.tys.subst(ret, args);
        concretize(st, ty)
    };
    (sp, sret, defaults)
}

fn member_lookup(st: &SemState, def: DefId, name: ast::interner::Symbol) -> Option<DefId> {
    crate::declare::member_lookup(st, def, name)
}

// ==================== 波次驱动（lib.rs analyze 调一次） ====================

pub(crate) fn run_waves(sem: &mut Sem) {
    loop {
        let jobs = std::mem::take(&mut sem.st.mono_queue);
        if jobs.is_empty() {
            break;
        }
        let start = sem.st.items_done;
        // 阶段一：克隆 + 重编号 + 登记（不碰侧表）
        for job in &jobs {
            if matches!(sem.st.defs.get(job.def).kind, DefKind::Fun) {
                stage_fun_clone(sem, job);
            }
        }
        if sem.st.items_done == sem.ast.items.len() {
            continue;
        }
        sem.st.items_done = sem.ast.items.len();
        // 侧表扩容必须在任何 sig lower / pass 之前（debug_assert 守护密度契约）
        grow_tables(&mut sem.st);
        // 阶段二：类型形参作用域 + 代入表 + 签名 lower
        for job in &jobs {
            if matches!(sem.st.defs.get(job.def).kind, DefKind::Fun) {
                finish_fun_clone(sem, job);
            }
        }
        let ast = &sem.ast;
        let st = &mut sem.st;
        crate::declare::run_from(ast, st, start);
        crate::resolve::run_from(ast, st, start);
        crate::check::run_from(ast, st, start);
        // 本波 TypeParam 兑现完毕（下一波的克隆会重建自己的代入表）
        st.mono_subst.clear();
    }
}

/// 阶段一：克隆 AST 重编号追加 + decl_def 登记（侧表扩容前的无侧表操作）
fn stage_fun_clone(sem: &mut Sem, job: &MonoJob) {
    let Some(decl_idx) = sem.st.defs.get(job.def).decl else {
        // 泛型 fun 必在顶层声明（v1 无嵌套 fun）；契约破缺时不克隆
        return;
    };
    let mut clone = sem.ast.items[decl_idx].clone();
    ast::syntax::renumber::renumber_decl(&mut clone, &mut sem.st.next_node);
    let item_idx = sem.ast.items.len();
    sem.ast.items.push(clone);
    let spec = sem.st.mono_insts[&(job.def, job.args.clone())];
    sem.st.decl_def.push(Some(spec));
    sem.st.defs.get_mut(spec).decl = Some(item_idx);
}

/// 阶段二：类型形参作用域 + 代入表 + spec def 签名 lower
/// （mono_subst 本波内有效，resolve/check 期兑现 T）
fn finish_fun_clone(sem: &mut Sem, job: &MonoJob) {
    let item_idx = sem
        .st
        .defs
        .get(sem.st.mono_insts[&(job.def, job.args.clone())])
        .decl
        .expect("阶段一已登记");
    let (span, scope, unit, error) = {
        let d = sem.st.defs.get(job.def);
        (d.span, d.scope, sem.st.tys.unit, sem.st.tys.error)
    };
    let spec = sem.st.mono_insts[&(job.def, job.args.clone())];
    let tscope = sem.st.scopes.new_scope(Some(scope));
    sem.st.defs.get_mut(spec).member_scope = Some(tscope);
    let DeclKind::Fun(f) = &sem.ast.items[item_idx].kind else {
        return;
    };
    for (i, tp) in f.type_params.iter().enumerate() {
        let TypeParamName::Valid(name) = &tp.name else {
            continue;
        };
        let tpdef = sem.st.defs.push(Def {
            kind: DefKind::TypeParam { index: i as u32 },
            name: *name,
            scope: tscope,
            span: tp.span,
            sig: DefSig::new(unit, error),
            resolving: false,
            alias_of: None,
            alias_resolved: None,
            member_scope: None,
            external: false,
            ty_params: 0,
            decl: None,
        });
        sem.st.scopes.insert(tscope, *name, tpdef);
        let arg = job.args.get(i).copied().unwrap_or(error);
        sem.st.mono_subst.push((tpdef, arg));
    }
    // 签名 lower：T 经 mono_subst 落成具体类型
    let params = crate::declare::lower_params(&mut sem.st, tscope, &f.params, span);
    let ret = f
        .ret
        .as_ref()
        .map(|t| crate::declare::lower_ty(&mut sem.st, tscope, t))
        .unwrap_or(unit);
    let defaults: Vec<bool> = f.params.iter().map(|p| p.default.is_some()).collect();
    let sig = &mut sem.st.defs.get_mut(spec).sig;
    sig.params = params;
    sig.ret = ret;
    sig.param_defaults = defaults;
}

/// 侧表扩容到 next_node（克隆的 NodeId 越过原始 node_count）
fn grow_tables(st: &mut SemState) {
    let n = st.next_node as usize;
    st.expr_ty.grow(n);
    st.expr_def.grow(n);
    st.pat_res.grow(n);
    st.flow_res.grow(n);
    st.node_scope.grow(n);
    st.ty_of_node.grow(n);
    st.pat_binding_def.grow(n);
}
