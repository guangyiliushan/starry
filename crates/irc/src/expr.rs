//! 表达式 lowering：每个 ExprWrap → Operand（Temp 或 Const）。
//! 类型取自 expr_ty 侧表；隐式提升与显式 as 统一走 `coerce` 插 Conv
//! （操作数类型均匀不变量，opt-notes）。

use ast::interner::Symbol;
use ast::syntax::expr::{Block, CallArg, CastKind, ExprKind, ExprWrap, LambdaParams};
use ast::syntax::node::{DiagCode, NodeId, Span};
use ast::syntax::pat::PatKind;
use ast::token::{LiteralKind, OperatorKind};
use sem::{DefId, DefKind, IntKind, TyId, TyKind};

use crate::lower::{Ctx, Place};
use crate::tac::{BinOp, Callee, Const, FunBody, Instr, Operand, RuntimeFn, TempId, Term, UnOp};

/// lower_jump 的目标种类（与 sem::FlowKind 对齐，避免公开依赖其私有语义）
#[derive(Debug, Clone, Copy)]
pub(crate) enum FlowTarget {
    Break,
    Continue,
}

fn lambda_lower(ctx: &mut Ctx, e: &ExprWrap, lparams: &LambdaParams, lbody: &Block) -> Operand {
    let Some(&ldef) = ctx.sem.lambda_defs.get(&e.id.0) else {
        return Operand::Const(Const::Unit);
    };
    let lsig = ctx.sem.defs.get(ldef).sig.clone();
    // S2/P2-3b：捕获表驱动装箱。允许：标量（含 i128 2 槽 WideCell）/
    // class（1 槽标量化指针）/ 可空 class（2 槽 env + flag 分支）；
    // 聚合整体归后续（聚合位 ARC 缺失，与 N2 同源）
    let caps: Vec<(Symbol, TyId)> = ctx
        .sem
        .lambda_caps
        .get(&e.id.0)
        .cloned()
        .unwrap_or_default();
    for (name, cty) in &caps {
        if !matches!(
            ctx.sem.tys.kind(*cty),
            TyKind::Int(IntKind::I4)
                | TyKind::Int(IntKind::I8)
                | TyKind::Int(IntKind::I16)
                | TyKind::Int(IntKind::I32)
                | TyKind::Int(IntKind::I64)
                | TyKind::Int(IntKind::I128)
                | TyKind::Float(_)
                | TyKind::Bool
                | TyKind::Char
                | TyKind::String
                | TyKind::Unit
        ) && !is_class_ty(ctx, *cty)
        {
            ctx.diag(
                e.span,
                format!(
                    "capture `{}` of this type lands in a later batch (scalar/class/nullable-class captures only; aggregate captures need aggregate-position ARC)",
                    ctx.sem.interner.resolve(*name)
                ),
            );
        }
    }
    let saved_p = std::mem::take(&mut ctx.fun.params);
    let saved_b = std::mem::take(&mut ctx.fun.blocks);
    let saved_d = ctx.fun.def;
    let saved_r = ctx.fun.ret;
    let saved_e = ctx.fun.entry;
    let saved_nb = ctx.next_block;
    // 块索引同属当前 fun 的地址空间——不随 lambda 换出会污染主函数
    // （第二个 lambda 的块 id 与残留条目碰撞 → 空块发射越界）
    let saved_bi = std::mem::take(&mut ctx.block_index);
    ctx.fun.def = ldef;
    ctx.fun.ret = lsig.ret;
    let entry = ctx.reserve_block();
    ctx.switch_to(entry);
    ctx.fun.entry = entry;
    ctx.env_push();
    let pnames: Vec<Symbol> = match lparams {
        LambdaParams::It => vec![ctx.sem.interner.intern("it")],
        // S2/P2-1 spike 修复：`(a, b)` 整体是单元素元组模式（= 参数表）——
        // 经 param_kinds 展平后取名（与 sem 绑定纪律一致）
        LambdaParams::Explicit(_) => lparams
            .param_kinds()
            .iter()
            .filter_map(|kind| match *kind {
                PatKind::Binding { name, .. } => Some(*name),
                _ => None,
            })
            .collect(),
    };
    // S2/P2-3b（修正 7）：env 首参（raw box ptr）——仅捕获形态；
    // 调用点 tag 分派传 raw 基址，lambda 体 OffsetPtr 读取同一约定
    let env_t = if caps.is_empty() {
        None
    } else {
        let t = ctx.temp();
        ctx.fun.params.push((t, ctx.sem.env_ref_ty));
        Some(t)
    };
    for (i, (_, ty)) in lsig.params.iter().enumerate() {
        let t = ctx.temp();
        ctx.fun.params.push((t, *ty));
        let pn = pnames.get(i).copied().unwrap_or(lsig.params[i].0);
        ctx.env_top().insert(pn, (Place::Temp(t), *ty));
    }
    if let Some(env_t) = env_t {
        // 捕获名 → env 槽读取（槽位游标，每捕获 1 槽：标量/class 均
        // 1 槽标量化指针；可空 class 待 nullable ABI 审计后启用）
        let mut slot = 1usize;
        for (name, cty) in &caps {
            let slots = crate::layout::scalar_slots(ctx, *cty);
            let addr = ctx.temp();
            ctx.emit(Instr::OffsetPtr {
                dst: addr,
                base: env_t,
                bytes: (8 * slot) as i64,
                ty: ctx.sem.tys.unit,
            });
            let v = ctx.temp();
            ctx.emit(Instr::LoadPtr {
                dst: v,
                ptr: addr,
                ty: *cty,
            });
            ctx.env_top().insert(*name, (Place::Temp(v), *cty));
            slot += slots;
        }
    }
    let tail = ctx.lower_block(lbody);
    ctx.finish_fun(tail, lbody.tail.as_deref());
    ctx.env_pop();
    let mut lfun = FunBody {
        def: saved_d,
        params: saved_p,
        ret: saved_r,
        blocks: saved_b,
        entry: saved_e,
    };
    std::mem::swap(&mut lfun, &mut ctx.fun);
    ctx.extra_funs.push(lfun);
    ctx.next_block = saved_nb;
    ctx.block_index = saved_bi;
    ctx.switch_to(saved_e);
    if caps.is_empty() {
        // 无捕获：裸 fn 指针（零分配；C 回调直通不变）
        return Operand::Const(Const::Fun(ldef));
    }
    // S2/P2-3b：捕获 lambda = ARC box + tag 位（llvme 侧 ptr|1）。
    // payload = [fn_ptr, caps...]；box 的 ARC（copy retain / scope
    // release）随批次 2/3——batch 1 值语义正确优先
    let fn_ty = ctx.expr_ty(e);
    let cap_vals: Vec<(Operand, TyId)> = caps
        .iter()
        .map(|(name, cty)| {
            let v = match ctx.lookup(*name) {
                Some((place, ty)) => read_place(ctx, place, ty, e.span),
                None => {
                    ctx.diag(e.span, "捕获名不在 lowering 环境（sem 契约破缺）");
                    Operand::Const(Const::Int(0))
                }
            };
            (v, *cty)
        })
        .collect();
    let box_t = ctx.temp();
    // S2/P2-3b-2：有 class 捕获时填 env_caps——llvme 合成 env 释放函数
    //（meta 逐 class 捕获 release）；纯标量捕获保持 meta=null（IR 不变）
    let has_class_cap = caps.iter().any(|(_, cty)| is_class_ty(ctx, *cty));
    ctx.emit(Instr::AllocClass {
        dst: box_t,
        bytes: crate::expr::ARC_HEADER_BYTES + (1 + caps.len() as i64) * 8,
        deinit: None,
        env_caps: has_class_cap.then(|| caps.iter().map(|(_, t)| *t).collect()),
        ty: fn_ty,
    });
    // S2/P2-3b：box_t 持 tag 位值（raw|1）——创建侧 GEP 从 tagged 基址
    // 起算。payload 布局 = alloc_object 返回的 raw 基址 + 槽 j*8：
    // fn_ptr 在 raw+0（dispatch 从 boxp+0 取），caps 自 raw+8。
    // tagged + bytes == raw + 槽址 → bytes = 槽址 - 1。
    let a0 = ctx.temp();
    ctx.emit(Instr::OffsetPtr {
        dst: a0,
        base: box_t,
        bytes: -1,
        ty: ctx.sem.tys.unit,
    });
    ctx.emit(Instr::StorePtr {
        ptr: a0,
        src: Operand::Const(Const::Fun(ldef)),
        ty: fn_ty,
    });
    let mut slot_j = 1usize;
    for (v, cty) in cap_vals.into_iter() {
        let cap_slots = crate::layout::scalar_slots(ctx, cty);
        // class 捕获 +1（box meta 释放时归还）；标量无动作
        if let Operand::Temp(t) = &v
            && is_class_ty(ctx, cty)
        {
            ctx.emit_class_retain(crate::lower::Place::Temp(*t), cty);
        }
        let a = ctx.temp();
        ctx.emit(Instr::OffsetPtr {
            dst: a,
            base: box_t,
            bytes: (8 * slot_j) as i64 - 1,
            ty: ctx.sem.tys.unit,
        });
        ctx.emit(Instr::StorePtr {
            ptr: a,
            src: v,
            ty: cty,
        });
        slot_j += cap_slots;
    }
    Operand::Temp(box_t)
}

pub(crate) fn lower_expr(ctx: &mut Ctx, e: &ExprWrap) -> Operand {
    if let ExprKind::Lambda {
        params: lp,
        body: lb,
    } = &e.kind
    {
        return lambda_lower(ctx, e, lp, lb);
    }
    match &e.kind {
        ExprKind::Literal(kind) => literal(ctx, *kind, e.span),
        ExprKind::Bool(b) => Operand::Const(Const::Bool(*b)),
        ExprKind::Null => nullable_null(ctx, e),
        ExprKind::Ident(name) => ident(ctx, *name, e.span, e.id),
        ExprKind::Path(path) => {
            let Some(last) = path.segments.last() else {
                ctx.diag(e.span, "空路径穿透 parser 门禁（内部不变量破缺）");
                return Operand::Const(Const::Unit);
            };
            match ctx
                .sem
                .expr_def
                .get(e.id)
                .copied()
                .map(|d| &ctx.sem.defs.get(d).kind)
            {
                Some(DefKind::EnumVariant { index, .. }) => variant_value(ctx, e, *index),
                _ if path.segments.len() == 1 => ident(ctx, last.name, e.span, e.id),
                _ => ident(ctx, last.name, e.span, e.id),
            }
        }
        ExprKind::Binary { op, lhs, rhs } => binary(ctx, e, *op, lhs, rhs),
        ExprKind::AddrOf { expr, .. } => addr_of(ctx, e, expr),
        ExprKind::Deref { expr } => deref_read(ctx, e, expr),
        ExprKind::Unary { op, operand } => unary(ctx, e, *op, operand),
        ExprKind::Cast { expr, kind, ty: _ } => cast(ctx, e, expr, *kind),
        ExprKind::Call {
            callee,
            type_args,
            args,
        } => call(ctx, e, callee, type_args, args),
        ExprKind::If {
            cond,
            then_block,
            else_block,
        } => lower_if(ctx, e, cond, then_block, else_block.as_deref()),
        ExprKind::Unsafe { block } | ExprKind::Comptime { block } => {
            // sem 按普通块定型/核查（semantic-notes）；lowering 同纪律
            match ctx.lower_block(block) {
                Some((op, ty)) => op_or_unit(ctx, op, ty),
                None => Operand::Const(Const::Unit),
            }
        }
        ExprKind::Loop { body, .. } => ctx.lower_loop_expr(e, body),
        ExprKind::Is {
            lhs,
            pattern,
            negated,
        } => lower_is(ctx, e, lhs, pattern, *negated),
        ExprKind::In { lhs, rhs, .. } => lower_in(ctx, e, lhs, rhs),
        ExprKind::When { subject, branches } => lower_when(ctx, e, subject.as_deref(), branches),
        ExprKind::Index {
            base,
            index,
            const_index,
        } => lower_index(ctx, e, base, index, *const_index),
        ExprKind::Member { base, safe, .. } => lower_member(ctx, e, base, *safe),
        ExprKind::Tuple(elems) => tuple_literal(ctx, e, elems),
        ExprKind::Array(elems) => array_literal(ctx, e, elems),
        ExprKind::ArrayRepeat { value, count } => array_repeat(ctx, e, value, *count),
        ExprKind::StringInterp { parts } => string_interp(ctx, e, parts),
        ExprKind::ForceUnwrap { expr } => unwrap(ctx, e, expr, ctx.expr_ty(e)),
        ExprKind::Lambda { .. }
        | ExprKind::Concurrent { .. }
        | ExprKind::Spawn(_)
        | ExprKind::Async { .. }
        | ExprKind::Await { .. }
        | ExprKind::Proceed
        | ExprKind::Super { .. }
        | ExprKind::MacroCall { .. } => {
            // sem v1 对这些构造本来就报 SemUnsupportedConstruct，门禁挡住；
            // 到达这里 = 契约破缺，诊断兜底
            ctx.diag(e.span, "sem 跳过清单构造穿透门禁（内部不变量破缺）");
            Operand::Const(Const::Unit)
        }
        ExprKind::Error(_) | ExprKind::Missing(_) => {
            ctx.diag(e.span, "错误/缺失表达式穿透 sem 门禁（内部不变量破缺）");
            Operand::Const(Const::Unit)
        }
    }
}

fn op_or_unit(ctx: &mut Ctx, op: Operand, ty: TyId) -> Operand {
    if ctx.is_unit(ty) {
        Operand::Const(Const::Unit)
    } else {
        op
    }
}

/// 标识符读取：Temp 直接用；Slot 标量发 Load，聚合逐槽拷贝出新基址
/// （var 读取 = 快照语义，sem v1 全拷贝）
/// M4 slice 4b (Plan C): class or ?class (unified ownership predicate)
pub(crate) fn classish_ty(ctx: &Ctx, ty: TyId) -> bool {
    crate::layout::is_class(ctx, ty)
        || matches!(ctx.sem.tys.kind(ty), TyKind::Nullable(inner) if crate::layout::is_class(ctx, *inner))
}

/// M4 slice 4b (Plan C): +1-origin expressions. Calls/null are always +1;
/// ?? and if with a class result are normalized per-branch (borrowed
/// branches retain) and therefore +1 as well.
pub(crate) fn plus1_origin(ctx: &Ctx, e: &ExprWrap) -> bool {
    match &e.kind {
        ExprKind::Call { .. } => true,
        ExprKind::Null => true,
        ExprKind::Binary { op, .. } if *op == OperatorKind::QuestionQuestion => {
            classish_ty(ctx, ctx.expr_ty(e))
        }
        ExprKind::If { .. } => classish_ty(ctx, ctx.expr_ty(e)),
        // S2/P2-3b-2：lambda 创建即 +1（box；裸 fn 无 ARC，take 无害）
        ExprKind::Lambda { .. } => true,
        _ => false,
    }
}

fn ident(ctx: &mut Ctx, name: ast::interner::Symbol, span: Span, id: NodeId) -> Operand {
    // M4 slice 4b (Plan C)：weak 绑定读取 = upgrade（sem 侧定型 ?P）——
    // starry_weak_upgrade 得 +1 对象或 null，物化 flag+payload
    let weak = ctx.sem.expr_def.get(id).is_some_and(|d| {
        matches!(
            ctx.sem.defs.get(*d).kind,
            DefKind::LocalBinding { weak: true, .. }
        )
    });
    match ctx.lookup(name) {
        Some((place, ty)) if weak => weak_upgrade_read(ctx, place, ty, span, id),
        Some((place, ty)) => read_place(ctx, place, ty, span),
        None => {
            // M8 slice 1：顶层 fun 名在值位置 = 函数引用常量
            if let Some(def) = ctx.sem.expr_def.get(id).copied()
                && matches!(ctx.sem.defs.get(def).kind, DefKind::Fun)
            {
                return Operand::Const(crate::tac::Const::Fun(def));
            }
            if ctx.unsupported_globals.contains(&name) {
                // 声明点已报 unsupported；使用点给同码稳定诊断（不再落契约
                // 破缺断言），返回哑基址避免级联（未初始化读取无害——诊断
                // 已门禁编译失败）
                ctx.diag(span, "this global is not lowered (M-later)");
                return Operand::Temp(ctx.temp());
            }
            ctx.diag(span, "标识符不在 lowering 环境（sem 契约破缺）");
            Operand::Const(Const::Int(0))
        }
    }
}

/// M4 slice 4b (Plan C)：weak 读取 lowering——upgrade 得 +1 对象指针
///（strong==0 则 null），物化为 ?P（flag = ptr != null）。结果是 +1
/// 临时值：绑定/返回 take 转移，语句末 release（drain_stmt_temps）。
fn weak_upgrade_read(ctx: &mut Ctx, place: Place, ty: TyId, span: Span, id: NodeId) -> Operand {
    let obj = read_place(ctx, place, ty, span);
    let Operand::Temp(obj_t) = obj else {
        ctx.diag(
            span,
            "weak binding storage is not a temp (internal invariant)",
        );
        return Operand::Const(Const::Unit);
    };
    let u = ctx.temp();
    ctx.emit(Instr::Param {
        a: Operand::Temp(obj_t),
        ty,
    });
    ctx.emit(Instr::Call {
        dst: Some(u),
        callee: Callee::Runtime(crate::tac::RuntimeFn::WeakUpgrade),
        ty, // Call 的 ty = 返回类型（ptr），非 unit
    });
    // flag = (u != null)：Bin 的 ty 字段 = 操作数类型（class -> ptr 比较）
    let flag = ctx.temp();
    ctx.emit(Instr::Bin {
        dst: flag,
        op: BinOp::Ne,
        l: Operand::Temp(u),
        r: Operand::Const(Const::Null),
        ty,
    });
    // sem 已把 weak 读取定型为 ?P（侧表取，TyArena 此处不可变）
    let nullable_ty = ctx.sem.expr_ty.get(id).copied().unwrap_or(ty);
    let out = ctx.temp();
    ctx.emit(Instr::Store {
        base: out,
        idx: Operand::Const(Const::Int(0)),
        src: Operand::Temp(flag),
        ty: ctx.sem.tys.bool_,
    });
    ctx.emit(Instr::Store {
        base: out,
        idx: Operand::Const(Const::Int(1)),
        src: Operand::Temp(u),
        ty,
    });
    ctx.note_stmt_temp(out, nullable_ty);
    Operand::Temp(out)
}

/// 读取值存放处为操作数：聚合 Slot → 逐槽拷贝出新基址（快照）
pub(crate) fn read_place(ctx: &mut Ctx, place: Place, ty: TyId, span: Span) -> Operand {
    match place {
        Place::Temp(t) => Operand::Temp(t),
        Place::Global(global) => {
            if ctx.is_unit(ty) {
                return Operand::Const(Const::Unit);
            }
            if crate::layout::is_aggregate(ctx, ty) {
                ctx.diag(span, "聚合全局读取的 lowering 在 M-later");
                return Operand::Const(Const::Unit);
            }
            let sty = crate::layout::slot_ty(ctx, ty, 0);
            let dst = ctx.temp();
            ctx.emit(Instr::LoadGlobal {
                dst,
                global,
                ty: sty,
            });
            Operand::Temp(dst)
        }
        Place::Slot(slot) => {
            if !crate::layout::is_aggregate(ctx, ty) {
                // 标量：槽值即值（槽类型，不带聚合型——llvme 的槽是定宽标量单元）
                let sty = crate::layout::slot_ty(ctx, ty, 0);
                let dst = ctx.temp();
                ctx.emit(Instr::Load {
                    dst,
                    base: slot,
                    idx: Operand::Const(Const::Int(0)),
                    ty: sty,
                });
                Operand::Temp(dst)
            } else {
                // 聚合（含单槽）：值 = 基址，逐槽拷贝出新基址（快照）
                let base = ctx.temp();
                copy_slots(ctx, (base, 0), (Operand::Temp(slot), 0), ty, ty, span);
                Operand::Temp(base)
            }
        }
    }
}

/// 从聚合基址的 off 槽起读取一个值：标量 → 槽类型 Load；聚合（含单槽）→
/// 逐槽拷贝出新基址（快照）。成员/索引/安全成员/模式字段读取共用
/// （read_place 的基址+偏移版）。
fn read_slots(ctx: &mut Ctx, base: TempId, off: usize, ty: TyId, span: Span) -> Operand {
    if !crate::layout::is_aggregate(ctx, ty) {
        let sty = crate::layout::slot_ty(ctx, ty, 0);
        let dst = ctx.temp();
        ctx.emit(Instr::Load {
            dst,
            base,
            idx: Operand::Const(Const::Int(off as i128)),
            ty: sty,
        });
        Operand::Temp(dst)
    } else {
        let b = ctx.temp();
        copy_slots(ctx, (b, 0), (Operand::Temp(base), off), ty, ty, span);
        Operand::Temp(b)
    }
}

/// 操作数必须是聚合基址 temp（构造/字面量的产物）；常量形态 = 契约破缺
fn as_base(ctx: &mut Ctx, op: Operand, span: Span) -> TempId {
    match op {
        Operand::Temp(t) => t,
        Operand::Const(_) => {
            ctx.diag(span, "聚合基址为常量（sem 契约破缺）");
            ctx.temp()
        }
    }
}

/// M4 slice 1: class value = object pointer (scalarized; see layout::is_aggregate)
pub(crate) fn is_class_ty(ctx: &Ctx, ty: TyId) -> bool {
    matches!(
        ctx.sem.tys.kind(ty),
        TyKind::Named(def, _) if matches!(ctx.sem.defs.get(*def).kind, DefKind::Class)
    )
}

/// M4 slice 1: ARC header bytes (strong/weak/meta, fixed early per D-004 ABI)
pub(crate) const ARC_HEADER_BYTES: i64 = 24;

/// 对象字段地址：OffsetPtr(obj, header + slot*8)。槽全 i64（sem 已拒
/// i128/聚合 class 字段），payload 偏移按 8B 槽计。
pub(crate) fn class_field_addr(ctx: &mut Ctx, obj: TempId, parent: DefId, index: usize) -> TempId {
    let slot = crate::layout::field_slot(ctx, parent, index) as i64;
    let addr = ctx.temp();
    ctx.emit(Instr::OffsetPtr {
        dst: addr,
        base: obj,
        bytes: ARC_HEADER_BYTES + slot * 8,
        ty: ctx.sem.tys.unit,
    });
    addr
}

/// M4 slice 1: class construction — heap alloc + payload field stores.
/// 构造者持 strong=1；槽机同 struct_ctor（位置/命名实参）。
fn class_ctor(ctx: &mut Ctx, e: &ExprWrap, def: DefId, args: &[CallArg]) -> Operand {
    let fields = ctx.sem.defs.get(def).sig.fields.clone();
    let payload = fields.len() as i64 * 8;
    let obj = ctx.temp();
    // M4 slice 2b：deinit 派发——class 带 deinit 成员时把函数指针
    // 塞进 header meta（release 末引用时回调）；无则 null
    let deinit = ctx.sem.defs.get(def).member_scope.and_then(|ms| {
        ctx.sem
            .scopes
            .lookup_local(ms, ctx.sem.interner.intern("deinit"))
    });
    let obj_ty = ctx.expr_ty(e);
    ctx.emit(Instr::AllocClass {
        dst: obj,
        bytes: ARC_HEADER_BYTES + payload,
        deinit,
        env_caps: None,
        ty: obj_ty,
    });
    // 主构造槽机：sig.params（头参数全量，含仅构造参数）；字段参数落
    // payload，仅构造参数求值后丢弃（M3 纪律）
    let ctor_params = ctx.sem.defs.get(def).sig.params.clone();
    let slot_of = |name| fields.iter().position(|(n, _)| *n == name);
    let mut slots: Vec<Option<Operand>> = vec![None; ctor_params.len()];
    let mut bad = false;
    for (i, arg) in args.iter().enumerate() {
        let idx = match arg.name {
            Some(n) => match ctor_params.iter().position(|(s, _)| *s == n) {
                Some(p) => p,
                None => {
                    ctx.diag(
                        e.span,
                        "named arg has no parameter (SemUnknownParam missed)",
                    );
                    bad = true;
                    continue;
                }
            },
            None => i,
        };
        if idx >= slots.len() || slots[idx].is_some() {
            ctx.diag(e.span, "ctor arg slot conflict (SemArityMismatch missed)");
            bad = true;
            continue;
        }
        let v = lower_expr(ctx, &arg.value);
        slots[idx] = Some(coerce(ctx, v, ctx.expr_ty(&arg.value), ctor_params[idx].1));
    }
    if !bad {
        for s in &slots {
            if s.is_none() {
                ctx.diag(e.span, "missing ctor arg (SemArityMismatch missed)");
            }
        }
    }
    for (idx, v) in slots.into_iter().enumerate() {
        if let Some(v) = v
            && let Some(fidx) = slot_of(ctor_params[idx].0)
        {
            let addr = class_field_addr(ctx, obj, def, fidx);
            ctx.emit(Instr::StorePtr {
                ptr: addr,
                src: v,
                ty: fields[fidx].1,
            });
        }
    }
    // M4 slice 4b (Plan C): ctor result is a +1 temp (consumers take;
    // otherwise released at statement end)
    ctx.note_stmt_temp(obj, obj_ty);
    Operand::Temp(obj)
}

/// 逐槽拷贝（带跨槽类型转换）：源值（类型 src_ty，位于 src 基址的
/// src_off 槽处）的第 s 槽经 coerce 写入 dst[dst_off+s]。slot_ty 的
/// 下标是值类型内部下标，与基址偏移无关（off 只参与 Load/Store 的
/// 绝对槽地址）。槽数不一致 = 契约破缺（诊断后置信下游不启动）。
/// 单槽快路径只对**标量**操作数免 Load；聚合源是基址，必须先取槽值。
/// 参数以 (基址, 槽偏移) 元组成对传入。
pub(crate) fn copy_slots(
    ctx: &mut Ctx,
    dst: (TempId, usize),
    src: (Operand, usize),
    src_ty: TyId,
    dst_ty: TyId,
    span: Span,
) {
    let (dst_base, dst_off) = dst;
    let (src, src_off) = src;
    let n = crate::layout::scalar_slots(ctx, dst_ty);
    if n != crate::layout::scalar_slots(ctx, src_ty) {
        ctx.diag(span, "槽数不一致的聚合拷贝（sem 契约破缺）");
        return;
    }
    if n <= 1 {
        let from_sty = crate::layout::slot_ty(ctx, src_ty, 0);
        let sty = crate::layout::slot_ty(ctx, dst_ty, 0);
        // 聚合源是基址（值 = 基址 + 槽），先取槽值再入槽；标量/常量即值
        let src_val = match &src {
            Operand::Temp(b) if crate::layout::is_aggregate(ctx, src_ty) => {
                let t = ctx.temp();
                ctx.emit(Instr::Load {
                    dst: t,
                    base: *b,
                    idx: Operand::Const(Const::Int(src_off as i128)),
                    ty: from_sty,
                });
                Operand::Temp(t)
            }
            _ => src.clone(),
        };
        let v = coerce(ctx, src_val, from_sty, sty);
        ctx.emit(Instr::Store {
            base: dst_base,
            idx: Operand::Const(Const::Int(dst_off as i128)),
            src: v,
            ty: sty,
        });
        return;
    }
    // S3：标量源（i128 成员等 n>1 的标量值）单成员 Store——llvme 按
    // ty=I128 split 到 lo/hi 两槽；聚合源才逐槽搬运
    let src_is_agg_base =
        matches!(&src, Operand::Temp(b) if crate::layout::is_aggregate(ctx, src_ty));
    if !src_is_agg_base {
        let sty = crate::layout::slot_ty(ctx, dst_ty, dst_off);
        let from_sty = crate::layout::slot_ty(ctx, src_ty, 0);
        let v = coerce(ctx, src.clone(), from_sty, sty);
        ctx.emit(Instr::Store {
            base: dst_base,
            idx: Operand::Const(Const::Int(dst_off as i128)),
            src: v,
            ty: sty,
        });
        return;
    }
    for s in 0..n {
        let from_sty = crate::layout::slot_ty(ctx, src_ty, s);
        let to_sty = crate::layout::slot_ty(ctx, dst_ty, s);
        let v = match &src {
            Operand::Temp(b) => {
                let t = ctx.temp();
                ctx.emit(Instr::Load {
                    dst: t,
                    base: *b,
                    idx: Operand::Const(Const::Int((src_off + s) as i128)),
                    ty: from_sty,
                });
                Operand::Temp(t)
            }
            Operand::Const(c) => Operand::Const(c.clone()),
        };
        let v = coerce(ctx, v, from_sty, to_sty);
        ctx.emit(Instr::Store {
            base: dst_base,
            idx: Operand::Const(Const::Int((dst_off + s) as i128)),
            src: v,
            ty: to_sty,
        });
    }
}

fn literal(ctx: &mut Ctx, kind: LiteralKind, span: Span) -> Operand {
    let text = span.slice(ctx.source);
    match kind {
        LiteralKind::Integer => Operand::Const(int_literal(ctx, text, span)),
        LiteralKind::Float => {
            let v: f64 = text
                .replace('_', "")
                .parse()
                .expect("sem 门禁后的浮点字面量必可解析");
            Operand::Const(Const::Float(v))
        }
        LiteralKind::String => {
            // 两种 span 形态：整字面量 token（带引号）与插值裸 chunk
            // （parser string_chunk 的 span 即去引号文本）
            let inner = if text.starts_with('"') {
                text.strip_prefix('"')
                    .and_then(|s| s.strip_suffix('"'))
                    .expect("lexer 保证引号配对")
            } else {
                text
            };
            let decoded = unescape(ctx, inner, span);
            Operand::Const(Const::Str(ctx.intern(&decoded)))
        }
        LiteralKind::Char => {
            let inner = text
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
                .expect("lexer 保证引号配对");
            let unescaped = unescape(ctx, inner, span);
            // char 在 LLVM 侧为 i32（ty_name）；取第一个标量值
            let c = unescaped.chars().next().unwrap_or('\0');
            Operand::Const(Const::Int(c as i128))
        }
    }
}

/// 整数字面量取值：词法规则含 0x/0o/0b 前缀（`lexer.rs` 规则表），
/// 且全仓无任何普通字面量的值域核查（IntegerOutOfRange 只覆盖数组
/// repeat 计数）——故解析与值域诊断都在本层：越界 i128 发
/// `IntegerOutOfRange` 并回退 0（诊断后置信下游不启动）。
fn int_literal(ctx: &mut Ctx, text: &str, span: Span) -> Const {
    let cleaned: String = text.replace('_', "");
    let (digits, radix) = match cleaned.as_str() {
        s if s.len() > 2 && s[..2].eq_ignore_ascii_case("0x") => (&s[2..], 16),
        s if s.len() > 2 && s[..2].eq_ignore_ascii_case("0o") => (&s[2..], 8),
        s if s.len() > 2 && s[..2].eq_ignore_ascii_case("0b") => (&s[2..], 2),
        s => (s, 10),
    };
    // 词法字符类保证 digits 合法（char 规则的 hex 位/数字位），
    // from_str_radix 只可能溢出——无 panic 路径
    match i128::from_str_radix(digits, radix) {
        Ok(v) => Const::Int(v),
        Err(_) => {
            ctx.diag_code(
                DiagCode::IntegerOutOfRange,
                span,
                "integer literal out of range",
            );
            Const::Int(0)
        }
    }
}

/// 转义序列解码，与 `parser::decode_string_token` 的接受集对齐：
/// `\n \t \r \0 \\ \" \'` 与 `\u{XXXXXX}`。未知转义保留原样——
/// 表达式字符串的词法校验是宽容的（`\\[^\r\n]` 任意字符），错误
/// 形态只可能来自 `\u{...}`，由本层发 `InvalidEscape`。
fn unescape(ctx: &mut Ctx, s: &str, span: Span) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some('u') => {
                // \u{XXXXXX}：词法对 char 字面量保证良构；字符串侧任意
                let mut hex = String::new();
                let mut closed = chars.next() == Some('{');
                if closed {
                    closed = false;
                    for c in chars.by_ref() {
                        if c == '}' {
                            closed = true;
                            break;
                        }
                        hex.push(c);
                    }
                }
                match if closed {
                    u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
                } else {
                    None
                } {
                    Some(decoded) => out.push(decoded),
                    None => {
                        ctx.diag_code(DiagCode::InvalidEscape, span, "invalid unicode escape");
                        out.push('\u{FFFD}');
                    }
                }
            }
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn binop(op: OperatorKind) -> Option<BinOp> {
    Some(match op {
        OperatorKind::Plus => BinOp::Add,
        OperatorKind::PlusPercent => BinOp::WrappingAdd,
        OperatorKind::Minus => BinOp::Sub,
        OperatorKind::MinusPercent => BinOp::WrappingSub,
        OperatorKind::Star => BinOp::Mul,
        OperatorKind::StarPercent => BinOp::WrappingMul,
        OperatorKind::Slash => BinOp::Div,
        OperatorKind::Percent => BinOp::Rem,
        OperatorKind::EqEq => BinOp::Eq,
        OperatorKind::BangEq => BinOp::Ne,
        OperatorKind::Lt => BinOp::Lt,
        OperatorKind::LtEq => BinOp::Le,
        OperatorKind::Gt => BinOp::Gt,
        OperatorKind::GtEq => BinOp::Ge,
        OperatorKind::BitAnd => BinOp::BitAnd,
        OperatorKind::BitOr => BinOp::BitOr,
        OperatorKind::BitXor => BinOp::BitXor,
        OperatorKind::Shl => BinOp::Shl,
        OperatorKind::Shr => BinOp::Shr,
        _ => return None,
    })
}

fn binary(
    ctx: &mut Ctx,
    e: &ExprWrap,
    op: OperatorKind,
    lhs: &ExprWrap,
    rhs: &ExprWrap,
) -> Operand {
    if op == OperatorKind::And || op == OperatorKind::Or {
        return materialize_logic(ctx, op, lhs, rhs);
    }
    if op == OperatorKind::QuestionQuestion {
        return lower_elvis(ctx, e, lhs, rhs);
    }
    if op == OperatorKind::DotDot || op == OperatorKind::DotDotEq {
        return range_value(ctx, e, lhs, rhs);
    }
    if op == OperatorKind::Plus && matches!(ctx.sem.tys.kind(ctx.expr_ty(e)), TyKind::String) {
        return string_concat(ctx, lhs, rhs);
    }
    let Some(bop) = binop(op) else {
        ctx.diag(e.span, "该二元运算符无 TAC 映射（sem 契约破缺）");
        return Operand::Const(Const::Unit);
    };
    let l_ty = ctx.expr_ty(lhs);
    let r_ty = ctx.expr_ty(rhs);
    // ty 字段 = 操作数类型（llvme 按算子推结果型：算术 = ty，比较 = i1）。
    // 比较的操作数公共型按 sem 提升格取可指派方向；算术直接用结果型
    // （sem join 已把结果定为操作数的公共型）
    let is_cmp = matches!(
        bop,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
    );
    // 操作数公共型：比较的结果型是 bool，操作数型须按提升格另算
    // （assignable(r, l) 时 l 就是公共型，否则取 r——sem 保证可汇合）
    let ty = if is_cmp {
        if ctx.sem.tys.assignable(r_ty, l_ty) {
            l_ty
        } else {
            r_ty
        }
    } else {
        ctx.expr_ty(e)
    };
    let l_op = lower_expr(ctx, lhs);
    let l = coerce(ctx, l_op, l_ty, ty);
    let r_op = lower_expr(ctx, rhs);
    let r = coerce(ctx, r_op, r_ty, ty);
    if is_cmp && matches!(bop, BinOp::Eq | BinOp::Ne) {
        // 字符串按内容比较（指针比较会静默错值）；聚合按结构相等
        //（文档承诺命名类型/数组/元组/区间结构相等）。
        let eq = if matches!(ctx.sem.tys.kind(ty), TyKind::String) {
            string_eq(ctx, l, r)
        } else if crate::layout::is_aggregate(ctx, ty) {
            structural_eq(ctx, l, r, ty)
        } else {
            let dst = ctx.temp();
            ctx.emit(Instr::Bin {
                dst,
                op: BinOp::Eq,
                l,
                r,
                ty,
            });
            Operand::Temp(dst)
        };
        return if bop == BinOp::Ne {
            not_bool(ctx, eq)
        } else {
            eq
        };
    }
    let dst = ctx.temp();
    ctx.emit(Instr::Bin {
        dst,
        op: bop,
        l,
        r,
        ty,
    });
    Operand::Temp(dst)
}

/// 短路布尔物化（龙书 6.6 跳转代码；Br 链，见 opt-notes）：
/// `l && r` / `l || r` → bool temp。
/// 结果 temp 是 phi 汇合物化：true/false 两分支块各写一次（多定义），
/// opt 侧据此永不判其循环不变（def_blocks 的 None 情形）
fn materialize_logic(ctx: &mut Ctx, op: OperatorKind, lhs: &ExprWrap, rhs: &ExprWrap) -> Operand {
    let is_and = op == OperatorKind::And;
    let result = ctx.temp();
    let bool_ty = ctx.expr_ty(lhs); // && 两侧恒 bool（sem SemConditionNotBool 门禁）

    let eval_rhs = ctx.reserve_block(); // 右侧求值块
    let done_true = ctx.reserve_block(); // result = true
    let done_false = ctx.reserve_block(); // result = false
    let merge = ctx.reserve_block();

    let l = lower_expr(ctx, lhs);
    let (then_, else_) = if is_and {
        (eval_rhs, done_false)
    } else {
        (done_true, eval_rhs)
    };
    ctx.terminate(Term::Br {
        cond: l,
        then_,
        else_,
    });

    ctx.switch_to(eval_rhs);
    let r = lower_expr(ctx, rhs);
    ctx.terminate(Term::Br {
        cond: r,
        then_: done_true,
        else_: done_false,
    });

    ctx.switch_to(done_true);
    ctx.emit(Instr::Copy {
        dst: result,
        src: Operand::Const(Const::Bool(true)),
        ty: bool_ty,
    });
    ctx.terminate(Term::Jmp(merge));

    ctx.switch_to(done_false);
    ctx.emit(Instr::Copy {
        dst: result,
        src: Operand::Const(Const::Bool(false)),
        ty: bool_ty,
    });
    ctx.terminate(Term::Jmp(merge));

    ctx.switch_to(merge);
    Operand::Temp(result)
}

fn string_concat(ctx: &mut Ctx, lhs: &ExprWrap, rhs: &ExprWrap) -> Operand {
    let str_ty = ctx.expr_ty(lhs);
    let l = lower_expr(ctx, lhs);
    let r = lower_expr(ctx, rhs);
    ctx.emit(Instr::Param { a: l, ty: str_ty });
    ctx.emit(Instr::Param { a: r, ty: str_ty });
    let dst = ctx.temp();
    ctx.emit(Instr::Call {
        dst: Some(dst),
        callee: Callee::Runtime(RuntimeFn::StrConcat),
        ty: str_ty,
    });
    Operand::Temp(dst)
}

/// 字符串内容相等（runtime `starry_str_eq`；指针比较是静默错值）
fn string_eq(ctx: &mut Ctx, l: Operand, r: Operand) -> Operand {
    let str_ty = ctx.sem.tys.string;
    ctx.emit(Instr::Param { a: l, ty: str_ty });
    ctx.emit(Instr::Param { a: r, ty: str_ty });
    let dst = ctx.temp();
    ctx.emit(Instr::Call {
        dst: Some(dst),
        callee: Callee::Runtime(RuntimeFn::StrEq),
        ty: ctx.sem.tys.bool_,
    });
    Operand::Temp(dst)
}

fn not_bool(ctx: &mut Ctx, v: Operand) -> Operand {
    let t = ctx.temp();
    ctx.emit(Instr::Un {
        dst: t,
        op: UnOp::Not,
        a: v,
        ty: ctx.sem.tys.bool_,
    });
    Operand::Temp(t)
}

fn load_slot(ctx: &mut Ctx, base: TempId, off: usize, ty: TyId) -> Operand {
    let t = ctx.temp();
    ctx.emit(Instr::Load {
        dst: t,
        base,
        idx: Operand::Const(Const::Int(off as i128)),
        ty,
    });
    Operand::Temp(t)
}

/// 从基址槽 offset 取子值：标量 Load，聚合经 copy_slots 拷到新块
/// （Load 只表示单槽，直接带聚合类型会把槽地址当值用）。
fn eq_value(ctx: &mut Ctx, base: TempId, off: usize, ty: TyId) -> Operand {
    if crate::layout::is_aggregate(ctx, ty) {
        let t = ctx.temp();
        crate::expr::copy_slots(
            ctx,
            (t, 0),
            (Operand::Temp(base), off),
            ty,
            ty,
            Span { start: 0, end: 0 },
        );
        Operand::Temp(t)
    } else {
        load_slot(ctx, base, off, ty)
    }
}

/// 聚合结构相等（semantic-notes：命名类型/数组/元组/区间按结构相等）。
/// 标量走 Bin Eq（浮点 fcmp），字符串走 runtime，聚合逐槽递归。
fn structural_eq(ctx: &mut Ctx, l: Operand, r: Operand, ty: TyId) -> Operand {
    if matches!(ctx.sem.tys.kind(ty), TyKind::String) {
        return string_eq(ctx, l, r);
    }
    if !crate::layout::is_aggregate(ctx, ty) {
        let dst = ctx.temp();
        ctx.emit(Instr::Bin {
            dst,
            op: BinOp::Eq,
            l,
            r,
            ty,
        });
        return Operand::Temp(dst);
    }
    let span = Span { start: 0, end: 0 };
    let lb = as_base(ctx, l, span);
    let rb = as_base(ctx, r, span);
    let kind = ctx.sem.tys.kind(ty).clone();
    match kind {
        TyKind::Array(elem, n) => {
            let es = crate::layout::scalar_slots(ctx, elem);
            let mut acc = Operand::Const(Const::Bool(true));
            for i in 0..n as usize {
                let a = eq_value(ctx, lb, i * es, elem);
                let b = eq_value(ctx, rb, i * es, elem);
                let e = structural_eq(ctx, a, b, elem);
                acc = and_operands(ctx, acc, e);
            }
            acc
        }
        TyKind::Tuple(elems) => {
            let mut off = 0;
            let mut acc = Operand::Const(Const::Bool(true));
            for e in elems {
                let a = eq_value(ctx, lb, off, e);
                let b = eq_value(ctx, rb, off, e);
                let c = structural_eq(ctx, a, b, e);
                acc = and_operands(ctx, acc, c);
                off += crate::layout::scalar_slots(ctx, e);
            }
            acc
        }
        TyKind::Range(inner) => {
            let a0 = eq_value(ctx, lb, 0, inner);
            let b0 = eq_value(ctx, rb, 0, inner);
            let c0 = structural_eq(ctx, a0, b0, inner);
            let n = crate::layout::scalar_slots(ctx, inner);
            let a1 = eq_value(ctx, lb, n, inner);
            let b1 = eq_value(ctx, rb, n, inner);
            let c1 = structural_eq(ctx, a1, b1, inner);
            let a2 = load_slot(ctx, lb, 2 * n, ctx.i8_ty);
            let b2 = load_slot(ctx, rb, 2 * n, ctx.i8_ty);
            let c2 = cmp_op(ctx, BinOp::Eq, a2, b2, ctx.i8_ty);
            let c12 = and_operands(ctx, c1, c2);
            and_operands(ctx, c0, c12)
        }
        TyKind::Nullable(inner) => nullable_eq(ctx, lb, rb, inner),
        TyKind::Named(def, _) => match ctx.sem.defs.get(def).kind {
            DefKind::Struct | DefKind::Class => {
                let fields = ctx.sem.defs.get(def).sig.fields.clone();
                let mut acc = Operand::Const(Const::Bool(true));
                for (i, (_, fty)) in fields.iter().enumerate() {
                    let off = crate::layout::field_slot(ctx, def, i);
                    let a = eq_value(ctx, lb, off, *fty);
                    let b = eq_value(ctx, rb, off, *fty);
                    let c = structural_eq(ctx, a, b, *fty);
                    acc = and_operands(ctx, acc, c);
                }
                acc
            }
            DefKind::Enum => enum_eq(ctx, lb, rb, def),
            _ => Operand::Const(Const::Bool(true)),
        },
        _ => Operand::Const(Const::Bool(true)),
    }
}

/// nullable 结构相等：flag 不同 → false；双 null → true；均非空 → payload 相等
fn nullable_eq(ctx: &mut Ctx, lb: TempId, rb: TempId, inner: TyId) -> Operand {
    let flag_ty = ctx.i8_ty;
    let fa = load_slot(ctx, lb, 0, flag_ty);
    let fb = load_slot(ctx, rb, 0, flag_ty);
    let result = ctx.temp();
    ctx.emit(Instr::Copy {
        dst: result,
        src: Operand::Const(Const::Bool(false)),
        ty: ctx.sem.tys.bool_,
    });
    let same = {
        let t = ctx.temp();
        ctx.emit(Instr::Bin {
            dst: t,
            op: BinOp::Eq,
            l: fa.clone(),
            r: fb.clone(),
            ty: flag_ty,
        });
        Operand::Temp(t)
    };
    let check_null = ctx.reserve_block();
    let is_null = ctx.reserve_block();
    let payload = ctx.reserve_block();
    let done = ctx.reserve_block();
    ctx.terminate(Term::Br {
        cond: same,
        then_: check_null,
        else_: done,
    });
    ctx.switch_to(check_null);
    let null_cmp = {
        let t = ctx.temp();
        ctx.emit(Instr::Bin {
            dst: t,
            op: BinOp::Eq,
            l: fa,
            r: Operand::Const(Const::Int(0)),
            ty: flag_ty,
        });
        Operand::Temp(t)
    };
    ctx.terminate(Term::Br {
        cond: null_cmp,
        then_: is_null,
        else_: payload,
    });
    ctx.switch_to(is_null);
    ctx.emit(Instr::Copy {
        dst: result,
        src: Operand::Const(Const::Bool(true)),
        ty: ctx.sem.tys.bool_,
    });
    ctx.terminate(Term::Jmp(done));
    ctx.switch_to(payload);
    let a = eq_value(ctx, lb, 1, inner);
    let b = eq_value(ctx, rb, 1, inner);
    let eq = structural_eq(ctx, a, b, inner);
    ctx.emit(Instr::Copy {
        dst: result,
        src: eq,
        ty: ctx.sem.tys.bool_,
    });
    ctx.terminate(Term::Jmp(done));
    ctx.switch_to(done);
    Operand::Temp(result)
}

/// 枚举结构相等：tag 不同 → false；tag 相同按该变体 payload 逐参比较。
fn enum_eq(ctx: &mut Ctx, lb: TempId, rb: TempId, def: sem::DefId) -> Operand {
    let tag_ty = ctx.i8_ty;
    let ta = load_slot(ctx, lb, 0, tag_ty);
    let tb = load_slot(ctx, rb, 0, tag_ty);
    let result = ctx.temp();
    ctx.emit(Instr::Copy {
        dst: result,
        src: Operand::Const(Const::Bool(false)),
        ty: ctx.sem.tys.bool_,
    });
    let same = {
        let t = ctx.temp();
        ctx.emit(Instr::Bin {
            dst: t,
            op: BinOp::Eq,
            l: ta.clone(),
            r: tb,
            ty: tag_ty,
        });
        Operand::Temp(t)
    };
    let variants = ctx.sem.defs.get(def).sig.variants.clone();
    if variants.is_empty() {
        return Operand::Const(Const::Bool(true));
    }
    let first = ctx.reserve_block();
    let false_b = ctx.reserve_block();
    let done = ctx.reserve_block();
    ctx.terminate(Term::Br {
        cond: same,
        then_: first,
        else_: false_b,
    });
    // 失败路径：result 已为 false
    ctx.switch_to(false_b);
    ctx.terminate(Term::Jmp(done));
    let mut current = first;
    for (idx, (_, params)) in variants.iter().enumerate() {
        ctx.switch_to(current);
        let is_v = {
            let t = ctx.temp();
            ctx.emit(Instr::Bin {
                dst: t,
                op: BinOp::Eq,
                l: ta.clone(),
                r: Operand::Const(Const::Int(idx as i128)),
                ty: tag_ty,
            });
            Operand::Temp(t)
        };
        let body = ctx.reserve_block();
        let next = if idx + 1 == variants.len() {
            false_b
        } else {
            ctx.reserve_block()
        };
        ctx.terminate(Term::Br {
            cond: is_v,
            then_: body,
            else_: next,
        });
        ctx.switch_to(body);
        let mut acc = Operand::Const(Const::Bool(true));
        for (pi, (_, pty)) in params.iter().enumerate() {
            let off = 1 + crate::layout::variant_param_slot(ctx, def, idx, pi);
            let a = eq_value(ctx, lb, off, *pty);
            let b = eq_value(ctx, rb, off, *pty);
            let c = structural_eq(ctx, a, b, *pty);
            acc = and_operands(ctx, acc, c);
        }
        ctx.emit(Instr::Copy {
            dst: result,
            src: acc,
            ty: ctx.sem.tys.bool_,
        });
        ctx.terminate(Term::Jmp(done));
        current = next;
    }
    ctx.switch_to(done);
    Operand::Temp(result)
}

fn unary(ctx: &mut Ctx, e: &ExprWrap, op: OperatorKind, operand: &ExprWrap) -> Operand {
    let uop = match op {
        OperatorKind::Minus => UnOp::Neg,
        OperatorKind::Bang => UnOp::Not,
        OperatorKind::BitNot => UnOp::BitNot,
        _ => {
            ctx.diag(e.span, "该一元运算符无 TAC 映射（sem 契约破缺）");
            return Operand::Const(Const::Unit);
        }
    };
    let ty = ctx.expr_ty(e);
    let a = lower_expr(ctx, operand);
    let dst = ctx.temp();
    ctx.emit(Instr::Un {
        dst,
        op: uop,
        a,
        ty,
    });
    Operand::Temp(dst)
}

/// as / as? / as!：`as` 数值转换走 Conv（全有符号）；as?/as! 依赖
/// nullable 槽布局，M2
fn cast(ctx: &mut Ctx, e: &ExprWrap, inner: &ExprWrap, kind: CastKind) -> Operand {
    match kind {
        CastKind::As => {
            let from = ctx.expr_ty(inner);
            let to = ctx.expr_ty(e);
            let op = lower_expr(ctx, inner);
            coerce(ctx, op, from, to)
        }
        CastKind::AsQuestion => nullable_cast(ctx, e, inner),
        CastKind::AsBang => unwrap(ctx, e, inner, ctx.expr_ty(e)),
    }
}

/// 隐式提升 + 显式窄化的统一出口：数值类型不同即插 Conv。
/// M2 取址 lowering：目标限局部名（sem 已拒全局/复合目标）。
/// Temp 绑定的地址 = 其槽地址；Slot 同理。AddrOf 指令由 llvme
/// 发为 alloca 地址（不透明指针）。
fn addr_of(ctx: &mut Ctx, e: &ExprWrap, target: &ExprWrap) -> Operand {
    let ExprKind::Ident(name) = &target.kind else {
        ctx.diag(e.span, "借用目标非局部名（sem 契约破缺）");
        return Operand::Const(Const::Null);
    };
    let Some((place, _)) = ctx.lookup(*name) else {
        ctx.diag(target.span, "借用目标不在 lowering 环境（sem 契约破缺）");
        return Operand::Const(Const::Null);
    };
    let base = match place {
        Place::Temp(t) | Place::Slot(t) => t,
        Place::Global(_) => {
            ctx.diag(target.span, "全局借用穿透 sem 门禁（契约破缺）");
            return Operand::Const(Const::Null);
        }
    };
    let dst = ctx.temp();
    ctx.emit(Instr::AddrOf {
        dst,
        base,
        ty: ctx.expr_ty(e),
    });
    Operand::Temp(dst)
}

/// M2 解引读：ptr 槽里存地址，经地址按 ty 读标量。
fn deref_read(ctx: &mut Ctx, e: &ExprWrap, target: &ExprWrap) -> Operand {
    let op = lower_expr(ctx, target);
    // 操作数的引用/指针型的内层 = 读出的槽类型（结果型即 inner，不再拆）
    let (TyKind::Ref { inner, .. } | TyKind::Pointer { inner, .. }) =
        ctx.sem.tys.kind(ctx.expr_ty(target)).clone()
    else {
        ctx.diag(e.span, "解引结果类型破缺（sem 契约破缺）");
        return Operand::Const(Const::Unit);
    };
    // S2/N2 审计（nullable-abi-audit.md §3.2）：解引指向可空的指针——
    // dst 按 Cell(i64) 分类 = flag 视图（payload 槽缺失，Some 值错）
    if matches!(ctx.sem.tys.kind(inner), TyKind::Nullable(..)) {
        ctx.diag(
            e.span,
            "dereferencing a pointer to a nullable type is not supported in v1 (single-slot load drops the payload)",
        );
        return Operand::Const(Const::Unit);
    }
    let ptr = match op {
        Operand::Temp(t) => t,
        Operand::Const(_) => {
            ctx.diag(target.span, "解引目标为常量（sem 契约破缺）");
            return Operand::Const(Const::Unit);
        }
    };
    let dst = ctx.temp();
    ctx.emit(Instr::LoadPtr {
        dst,
        ptr,
        ty: inner,
    });
    Operand::Temp(dst)
}

/// 非数值的 from≠to 由 sem 门禁保证不发生（到达即契约破缺，原样透传）。
pub(crate) fn coerce(ctx: &mut Ctx, op: Operand, from: TyId, to: TyId) -> Operand {
    if from == to || ctx.is_unit(to) || ctx.is_unit(from) {
        return op;
    }
    // 常量快路径：值的类型随指令走（操作数均匀不变量的常量侧），同类
    // 数值（整→整、浮→浮）无需 Conv；跨类（整↔浮）必须留 Conv，否则
    // llvme 会把整数印成浮点字面量
    if matches!(op, Operand::Const(_)) {
        let same_class = matches!(
            (ctx.sem.tys.kind(from), ctx.sem.tys.kind(to)),
            (TyKind::Int(_), TyKind::Int(_)) | (TyKind::Float(_), TyKind::Float(_))
        );
        if same_class {
            return op;
        }
    }
    // 聚合逐槽转换（数组/元组/区间元素级加宽）：分配新块，逐槽
    // Load → Conv → Store。旧实现对聚合直接透传，造成 4 字节 store /
    // 8 字节 load 的静默错值。
    if let Some(out) = coerce_aggregate(ctx, &op, from, to) {
        return out;
    }
    // M7 slice 2：泛型 enum 原件 → 实例化 spec（零参变体构造的布局
    // 与 T 无关；标注 `Opt<int>` 已在 lower_ty 期实例化 spec——
    // 返回目标即可，None 的 flag=0 布局与 T 无关）
    if let (TyKind::Named(fd, fa), TyKind::Named(td, _)) =
        (ctx.sem.tys.kind(from).clone(), ctx.sem.tys.kind(to).clone())
        && fa.is_empty()
        && fd != td
        && matches!(ctx.sem.defs.get(fd).kind, DefKind::Enum)
        && matches!(ctx.sem.defs.get(td).kind, DefKind::Enum)
        && ctx.sem.defs.get(td).ty_params == 0
        && ctx.sem.defs.get(fd).name == ctx.sem.defs.get(td).name
    {
        return op;
    }
    // Null → 裸指针：0 地址常量。不能先落 nullable_null——它的 flag
    // 形值只有 1 字节，按 ptr 加载会读到未初始化高位（指针 FFI 探针
    // 抓到 is_null(null) 收到非空垃圾）
    if matches!(ctx.sem.tys.kind(from), TyKind::Null)
        && matches!(ctx.sem.tys.kind(to), TyKind::Pointer { .. })
    {
        return Operand::Const(Const::Null);
    }
    // Null → Nullable：null 值（flag=0 + 清零 payload），先于通用
    // 值→Nullable 臂——否则 flag=1 把 null 传成非空（f(null) 探针抓到）
    if matches!(ctx.sem.tys.kind(from), TyKind::Null)
        && matches!(ctx.sem.tys.kind(to), TyKind::Nullable(_))
    {
        let TyKind::Nullable(inner) = ctx.sem.tys.kind(to).clone() else {
            unreachable!()
        };
        let base = ctx.temp();
        ctx.emit(Instr::Store {
            base,
            idx: Operand::Const(Const::Int(0)),
            src: Operand::Const(Const::Int(0)),
            ty: ctx.i8_ty,
        });
        if !ctx.is_unit(inner) {
            ctx.emit(Instr::Store {
                base,
                idx: Operand::Const(Const::Int(1)),
                src: Operand::Const(Const::Int(0)),
                ty: inner,
            });
        }
        return Operand::Temp(base);
    }
    // 值 → Nullable：显式构造 flag=1 + payload。不能落 copy_slots 的
    // 多槽 Const 路径（它把同一常量写进每个槽：flag=4/payload=4——
    // native 侧 br 按 i1 低比特判真直接走错分支，差分 harness 抓到）
    if let TyKind::Nullable(inner) = ctx.sem.tys.kind(to).clone()
        && !matches!(ctx.sem.tys.kind(from), TyKind::Nullable(_))
    {
        // M4 slice 4b (Plan C): transfer an old +1 temp; a borrowed class
        // must retain its payload so every release stays paired.
        let classish = classish_ty(ctx, inner);
        let transferred = classish && ctx.take_stmt_temp(&op);
        let base = ctx.temp();
        ctx.emit(Instr::Store {
            base,
            idx: Operand::Const(Const::Int(0)),
            src: Operand::Const(Const::Int(1)),
            ty: ctx.i8_ty,
        });
        let v = coerce(ctx, op, from, inner);
        if crate::layout::is_aggregate(ctx, inner) {
            copy_slots(
                ctx,
                (base, 1),
                (v, 0),
                inner,
                inner,
                Span { start: 0, end: 0 },
            );
        } else {
            ctx.emit(Instr::Store {
                base,
                idx: Operand::Const(Const::Int(1)),
                src: v,
                ty: inner,
            });
        }
        if classish {
            if !transferred {
                ctx.emit_class_retain(Place::Temp(base), to);
            }
            ctx.note_stmt_temp(base, to);
        }
        return Operand::Temp(base);
    }
    let numeric = matches!(
        (ctx.sem.tys.kind(from), ctx.sem.tys.kind(to)),
        (TyKind::Int(_), TyKind::Int(_))
            | (TyKind::Int(_), TyKind::Float(_))
            | (TyKind::Float(_), TyKind::Int(_))
            | (TyKind::Float(_), TyKind::Float(_))
    );
    if !numeric {
        return op;
    }
    let dst = ctx.temp();
    ctx.emit(Instr::Un {
        dst,
        op: UnOp::Conv(from),
        a: op,
        ty: to,
    });
    Operand::Temp(dst)
}

/// 聚合 coerce：仅处理槽结构一致（数组长度/元组元数/区间）的类型对，
/// 逐槽做标量转换。不适用时返回 None，由调用方按原规则处理。
fn coerce_aggregate(ctx: &mut Ctx, op: &Operand, from: TyId, to: TyId) -> Option<Operand> {
    let ok = match (ctx.sem.tys.kind(from).clone(), ctx.sem.tys.kind(to).clone()) {
        (TyKind::Array(fe, fl), TyKind::Array(te, tl)) => {
            fl == tl && crate::layout::scalar_slots(ctx, fe) == crate::layout::scalar_slots(ctx, te)
        }
        (TyKind::Tuple(fs), TyKind::Tuple(ts)) => {
            fs.len() == ts.len()
                && fs.iter().zip(&ts).all(|(f, t)| {
                    crate::layout::scalar_slots(ctx, *f) == crate::layout::scalar_slots(ctx, *t)
                })
        }
        (TyKind::Range(f), TyKind::Range(t)) => {
            crate::layout::scalar_slots(ctx, f) == crate::layout::scalar_slots(ctx, t)
        }
        _ => false,
    };
    if !ok {
        return None;
    }
    let n = crate::layout::scalar_slots(ctx, to);
    if n == 0 {
        return Some(op.clone());
    }
    let span = Span { start: 0, end: 0 };
    let src = as_base(ctx, op.clone(), span);
    let dst = ctx.temp();
    for i in 0..n {
        let fty = crate::layout::slot_ty(ctx, from, i);
        let tty = crate::layout::slot_ty(ctx, to, i);
        let v = load_slot(ctx, src, i, fty);
        let v = coerce(ctx, v, fty, tty);
        ctx.emit(Instr::Store {
            base: dst,
            idx: Operand::Const(Const::Int(i as i128)),
            src: v,
            ty: tty,
        });
    }
    Some(Operand::Temp(dst))
}

/// 按已知目标类型 lowering（初始化/赋值/实参/返回的「lower → coerce」
/// 调用点统一走这里）。数组/元组字面量直写目标元素宽：消除「字面量
/// 按自身窄宽初始化 + coerce_aggregate 二次 widen 双块」形态——窄写宽读
/// 是静默错编源（2026-11 审查 P0-1：`[int; 2] = [7, 8]` 曾在 [2 x i64]
/// 块里 store i32）。其余表达式与原路径一致（lower_expr + coerce）。
pub(crate) fn lower_expr_as(ctx: &mut Ctx, e: &ExprWrap, to: TyId) -> Operand {
    let from = ctx.expr_ty(e);
    if from == to {
        return lower_expr(ctx, e);
    }
    match (&e.kind, ctx.sem.tys.kind(to).clone()) {
        (ExprKind::Array(elems), TyKind::Array(telem, _)) => {
            let es = crate::layout::scalar_slots(ctx, telem);
            let base = ctx.temp();
            let mut off = 0;
            for el in elems {
                let v = lower_expr_as(ctx, el, telem);
                copy_slots(ctx, (base, off), (v, 0), telem, telem, e.span);
                off += es;
            }
            Operand::Temp(base)
        }
        (ExprKind::Tuple(elems), TyKind::Tuple(ttys)) => {
            let base = ctx.temp();
            let mut off = 0;
            for (i, el) in elems.iter().enumerate() {
                let ety = ttys.get(i).copied().unwrap_or(to);
                let v = lower_expr_as(ctx, el, ety);
                copy_slots(ctx, (base, off), (v, 0), ety, ety, e.span);
                off += crate::layout::scalar_slots(ctx, ety);
            }
            Operand::Temp(base)
        }
        _ => {
            let op = lower_expr(ctx, e);
            coerce(ctx, op, from, to)
        }
    }
}

/// M8 slice 1：函数值调用——callee 是持有 (..)->.. 值的形参/绑定。
/// 首 pending 实参为函数指针，llvme 发间接调用。
fn fn_value_call(
    ctx: &mut Ctx,
    e: &ExprWrap,
    callee: &ExprWrap,
    args: &[ast::syntax::expr::CallArg],
) -> Operand {
    let (ps, ret) = match ctx.sem.tys.kind(ctx.expr_ty(callee)).clone() {
        TyKind::Fn(ps, r) => (ps, r),
        _ => {
            ctx.diag(callee.span, "函数值调用目标非函数类型（sem 契约破缺）");
            return Operand::Const(Const::Unit);
        }
    };
    if args.len() != ps.len() {
        ctx.diag(e.span, "函数值调用 arity 不符（sem 契约破缺）");
    }
    if args.iter().any(|a| a.name.is_some()) {
        ctx.diag(e.span, "函数值调用不支持命名实参（sem 已拒）");
    }
    // S2/P2-1 spike 修复：先降全部实参再压 param——pending 队列在
    // 嵌套调用（f(f(x))）时，外层 callee 的 param 先入队会被内层
    // call 整队吞掉（实参临时槽不受 emit 顺序影响）
    let arg_vals: Vec<(Operand, TyId)> = args
        .iter()
        .enumerate()
        .map(|(i, arg)| {
            let to = ps.get(i).copied().unwrap_or(ctx.sem.tys.unit);
            let v = lower_expr_as(ctx, &arg.value, to);
            (v, to)
        })
        .collect();
    let fty = ctx.expr_ty(callee);
    let fop = lower_expr(ctx, callee);
    ctx.emit(Instr::Param { a: fop, ty: fty });
    for (v, to) in arg_vals {
        ctx.emit(Instr::Param { a: v, ty: to });
    }
    let dst = if ctx.is_unit(ret) {
        None
    } else {
        Some(ctx.temp())
    };
    ctx.emit(Instr::Call {
        dst,
        callee: Callee::Indirect,
        ty: ret,
    });
    match dst {
        Some(d) => Operand::Temp(d),
        None => Operand::Const(Const::Unit),
    }
}
fn call(
    ctx: &mut Ctx,
    e: &ExprWrap,
    callee: &ExprWrap,
    _type_args: &[ast::syntax::ty::TypeArg],
    args: &[ast::syntax::expr::CallArg],
) -> Operand {
    // M7：类型实参的消纳在 sem——泛型调用的 expr_def 已指向实例化
    // spec def（克隆体/具体 sig），本层按普通调用 lowering
    // callee 分派：expr_def 优先；变体构造等路径 expr_def 缺失——
    // callee 为成员访问时按名本地解析（layout::resolve_member）
    let def = ctx
        .sem
        .expr_def
        .get(e.id)
        .or_else(|| ctx.sem.expr_def.get(callee.id))
        .copied();
    if def.is_none()
        && let ExprKind::Member { base, name, .. } = &callee.kind
        && let Some(res) = crate::layout::resolve_member(ctx, ctx.expr_ty(base), *name)
    {
        match res {
            crate::layout::MemberRes::Variant { parent, index } => {
                return variant_ctor(ctx, e, parent, index, args);
            }
            crate::layout::MemberRes::Field { .. } => {
                ctx.diag(e.span, "字段名被当作构造器调用（sem 契约破缺）");
                return Operand::Const(Const::Unit);
            }
        }
    }
    // S2/P2-3b-3：柯里化调用——callee 是 Fn 类型的表达式（调用结果 /
    // 括号表达式等，无 expr_def）→ 函数值间接调用
    if def.is_none() && matches!(ctx.sem.tys.kind(ctx.expr_ty(callee)), TyKind::Fn(..)) {
        return fn_value_call(ctx, e, callee, args);
    }
    let Some(mut def) = def else {
        ctx.diag(e.span, "调用目标无 expr_def（sem 契约破缺）");
        return Operand::Const(Const::Unit);
    };
    if let DefKind::TypeAlias = ctx.sem.defs.get(def).kind {
        def = match ctx.sem.defs.get(def).alias_resolved {
            Some(target) => target,
            None => {
                ctx.diag(e.span, "别名无预解析目标（sem 契约破缺）");
                return Operand::Const(Const::Unit);
            }
        };
    }
    match ctx.sem.defs.get(def).kind {
        DefKind::BuiltinPrint => return runtime_print(ctx, e, args),
        DefKind::Struct => return struct_ctor(ctx, e, def, args),
        DefKind::Class => return class_ctor(ctx, e, def, args),
        DefKind::EnumVariant { parent, index } => {
            return variant_ctor(ctx, e, parent, index, args);
        }
        DefKind::Fun => {}
        _ => {
            // M8 slice 1：形参/绑定持有的函数值 → 间接调用
            if matches!(ctx.sem.tys.kind(ctx.expr_ty(callee)), TyKind::Fn(..)) {
                return fn_value_call(ctx, e, callee, args);
            }
            ctx.diag(e.span, "该调用目标无 lowering 路径（sem 契约破缺）");
            return Operand::Const(Const::Unit);
        }
    }
    // M4 slice 2b：class 方法调用——self（对象指针）作首实参注入。
    // callee 为成员访问且走到此处 = 方法（变体/字段路径已先行返回）；
    // sig.ty 是 sem 的 self 标记（class 值类型）
    if let ExprKind::Member { base, .. } = &callee.kind {
        let class_ty = ctx.sem.defs.get(def).sig.ty;
        let bop = lower_expr(ctx, base);
        ctx.emit(Instr::Param {
            a: bop,
            ty: class_ty,
        });
    }
    // 克隆签名槽（sem 审查已认可 per-call 克隆的规模；避免借用冲突）
    let (param_tys, param_defaults, ret) = {
        let sig = &ctx.sem.defs.get(def).sig;
        (sig.params.clone(), sig.param_defaults.clone(), sig.ret)
    };
    let mut slots: Vec<Option<Operand>> = vec![None; param_tys.len()];
    let mut dup_or_unknown = false;
    for (i, arg) in args.iter().enumerate() {
        let idx = match arg.name {
            Some(n) => match param_tys.iter().position(|(s, _)| *s == n) {
                Some(p) => p,
                None => {
                    ctx.diag(e.span, "命名实参无对应形参（SemUnknownParam 漏报）");
                    dup_or_unknown = true;
                    continue;
                }
            },
            None => i,
        };
        if idx >= slots.len() || slots[idx].is_some() {
            ctx.diag(e.span, "实参槽位冲突（SemArityMismatch 漏报）");
            dup_or_unknown = true;
            continue;
        }
        let v = lower_expr_as(ctx, &arg.value, param_tys[idx].1);
        slots[idx] = Some(v);
    }
    // 默认值补位：在调用点求值 callee 的默认表达式
    if !dup_or_unknown {
        for idx in 0..slots.len() {
            if slots[idx].is_none() {
                if !param_defaults.get(idx).copied().unwrap_or(false) {
                    ctx.diag(e.span, "缺少实参且无默认值（SemArityMismatch 漏报）");
                    continue;
                }
                let Some(decl) = ctx.find_fun_decl(def) else {
                    continue;
                };
                let Some(default) = decl.params.get(idx).and_then(|p| p.default.as_deref()) else {
                    ctx.diag(e.span, "默认实参 AST 缺失（sem 契约破缺）");
                    continue;
                };
                let v = lower_expr_as(ctx, default, param_tys[idx].1);
                slots[idx] = Some(v);
            }
        }
    }
    // S2/P2-3b（修正 7）：C 回调边界——只允许裸 fn 直通（Const::Fun）；
    // box（tagged ptr）过 extern 会把 tag 位当代码地址调用（矩阵 #16）
    if ctx.sem.defs.get(def).external {
        for (idx, slot) in slots.iter().enumerate() {
            if let Some(v) = slot
                && matches!(ctx.sem.tys.kind(param_tys[idx].1), TyKind::Fn(..))
                && !matches!(v, Operand::Const(Const::Fun(_)))
            {
                ctx.diag(
                    e.span,
                    "captured lambda cannot cross the extern \"c\" boundary (bare fn only)",
                );
            }
        }
    }
    for (idx, slot) in slots.into_iter().enumerate() {
        if let Some(v) = slot {
            ctx.emit(Instr::Param {
                a: v,
                ty: param_tys[idx].1,
            });
        }
    }
    let dst = if ctx.is_unit(ret) {
        None
    } else {
        Some(ctx.temp())
    };
    // M4 slice 4b (Plan C): class/?class return is a +1 temp
    if let Some(d) = dst
        && classish_ty(ctx, ret)
    {
        ctx.note_stmt_temp(d, ret);
    }
    let callee_op = if ctx.sem.defs.get(def).external {
        Callee::Extern(def)
    } else {
        Callee::Def(def)
    };
    ctx.emit(Instr::Call {
        dst,
        callee: callee_op,
        ty: ret,
    });
    match dst {
        Some(t) => Operand::Temp(t),
        None => Operand::Const(Const::Unit),
    }
}

fn runtime_print(ctx: &mut Ctx, e: &ExprWrap, args: &[ast::syntax::expr::CallArg]) -> Operand {
    if args.len() != 1 {
        ctx.diag(e.span, "print 实参数不匹配（SemArityMismatch 漏报）");
        return Operand::Const(Const::Unit);
    }
    let ty = ctx.expr_ty(&args[0].value);
    // S3：标量判据 = 类型种类（i128 聚合成员 loads 后是单槽 WideCell 标量值，
    // 槽数 2 不代表聚合——见 i128-layout-spec §3）
    if crate::layout::is_aggregate(ctx, ty) || matches!(ctx.sem.tys.kind(ty), TyKind::Nullable(_)) {
        ctx.diag(
            args[0].value.span,
            "print 参数类型无 runtime 入口（sem 契约破缺）",
        );
        return Operand::Const(Const::Unit);
    }
    let v = lower_expr(ctx, &args[0].value);
    ctx.emit(Instr::Param { a: v, ty });
    let runtime = if matches!(ctx.sem.tys.kind(ty), TyKind::Char) {
        RuntimeFn::PrintChar
    } else {
        RuntimeFn::Print
    };
    ctx.emit(Instr::Call {
        dst: None,
        callee: Callee::Runtime(runtime),
        ty: ctx.sem.tys.unit,
    });
    Operand::Const(Const::Unit)
}

/// M4 slice 4b (Plan C)：if 分支结果落槽。class 结果在分支内归一化为
/// +1：+1-origin 尾式转移（take），借用尾式显式 retain；语句位
///（slot=None）的 +1 尾式在分支末随 drain 释放（路径局部，正确）。
fn store_branch_result(
    ctx: &mut Ctx,
    slot: Option<TempId>,
    tail: Option<(Operand, TyId)>,
    tail_expr: Option<&ExprWrap>,
    result_ty: TyId,
    span: Span,
) {
    let Some((op, from)) = tail else { return };
    let Some(slot) = slot else {
        // statement position: a +1 tail temp dies at the branch end
        ctx.drain_stmt_temps();
        return;
    };
    if crate::layout::is_class(ctx, result_ty) {
        match tail_expr {
            Some(te) if plus1_origin(ctx, te) => {
                let _ = ctx.take_stmt_temp(&op);
            }
            Some(_) => {
                if let Operand::Temp(t) = op {
                    ctx.emit_class_retain(crate::lower::Place::Temp(t), result_ty);
                }
            }
            None => ctx.diag(span, "class branch tail AST missing (sem invariant)"),
        }
        ctx.emit(Instr::Store {
            base: slot,
            idx: Operand::Const(Const::Int(0)),
            src: op,
            ty: result_ty,
        });
        return;
    }
    if crate::layout::is_aggregate(ctx, result_ty) {
        copy_slots(ctx, (slot, 0), (op, 0), from, result_ty, span);
    } else {
        let v = coerce(ctx, op, from, result_ty);
        ctx.emit(Instr::Store {
            base: slot,
            idx: Operand::Const(Const::Int(0)),
            src: v,
            ty: result_ty,
        });
    }
}

fn lower_if(
    ctx: &mut Ctx,
    e: &ExprWrap,
    cond: &ast::syntax::expr::Condition,
    then_block: &ast::syntax::expr::Block,
    else_block: Option<&ast::syntax::expr::Block>,
) -> Operand {
    let value_ctx = !ctx.is_unit(ctx.expr_ty(e));
    let result_ty = ctx.expr_ty(e);

    let then_b = ctx.reserve_block();
    let else_b = ctx.reserve_block();
    let merge_b = ctx.reserve_block();

    // val/is 条件的模式绑定限 then 分支（scoped=true 时 lower_cond 已推作用域）
    let (c, scoped) = lower_cond(ctx, cond);
    // M4 slice 4b (Plan C)：条件里的 +1 class 临时值在分支前释放
    ctx.drain_stmt_temps();
    ctx.terminate(Term::Br {
        cond: c,
        then_: then_b,
        else_: else_b,
    });

    // 值语境：结果槽（两分支 Store、汇合 Load——用内存绕开 phi）
    let result_slot = if value_ctx { Some(ctx.temp()) } else { None };

    ctx.switch_to(then_b);
    let then_tail = ctx.lower_block(then_block);
    if scoped {
        ctx.env_pop();
    }
    // 聚合结果逐槽拷贝（单槽 Store 的 ty 必须是槽类型，不是聚合型）
    let span = e.span;
    store_branch_result(
        ctx,
        result_slot,
        then_tail,
        then_block.tail.as_deref(),
        result_ty,
        span,
    );
    ctx.terminate(Term::Jmp(merge_b));

    ctx.switch_to(else_b);
    if let Some(b) = else_block {
        let tail = ctx.lower_block(b);
        store_branch_result(ctx, result_slot, tail, b.tail.as_deref(), result_ty, span);
    }
    ctx.terminate(Term::Jmp(merge_b));

    ctx.switch_to(merge_b);
    match result_slot {
        Some(slot) => {
            if crate::layout::is_aggregate(ctx, result_ty) {
                // 槽基址即结果基址（val 不可变，无重赋值）
                Operand::Temp(slot)
            } else {
                let dst = ctx.temp();
                ctx.emit(Instr::Load {
                    dst,
                    base: slot,
                    idx: Operand::Const(Const::Int(0)),
                    ty: result_ty,
                });
                // M4 slice 4b (Plan C)：class 汇合结果统一 +1
                if crate::layout::is_class(ctx, result_ty) {
                    ctx.note_stmt_temp(dst, result_ty);
                }
                Operand::Temp(dst)
            }
        }
        None => Operand::Const(Const::Unit),
    }
}

// ---- 聚合值 lowering（M2a：槽位布局）----

/// null：和类型特例——只写 flag 槽 0（payload 不初始化，flag=0 时永不读）
fn nullable_null(ctx: &mut Ctx, e: &ExprWrap) -> Operand {
    let _ = e;
    let base = ctx.temp();
    ctx.emit(Instr::Store {
        base,
        idx: Operand::Const(Const::Int(0)),
        src: Operand::Const(Const::Int(0)),
        ty: ctx.i8_ty,
    });
    Operand::Temp(base)
}

/// 无参枚举变体值 E.A：tag 槽写入变体序号
fn variant_value(ctx: &mut Ctx, e: &ExprWrap, index: usize) -> Operand {
    if index > 255 {
        ctx.diag_code(
            DiagCode::IrcTooManyVariants,
            e.span,
            "变体序号超 tag 槽 i8 上限（>255 变体的枚举无法 lowering）",
        );
    }
    let base = ctx.temp();
    ctx.emit(Instr::Store {
        base,
        idx: Operand::Const(Const::Int(0)),
        src: Operand::Const(Const::Int(index as i128)),
        ty: ctx.i8_ty,
    });
    Operand::Temp(base)
}

/// M3 主构造调用：实参按参数名入槽；val/var 参数按名落字段槽（字段序与
/// 注册序一致），无修饰符参数仅求值（副作用保留、值丢弃）；缺省由
/// 声明期默认表达式在调用点 lowering。
fn primary_ctor_call(
    ctx: &mut Ctx,
    e: &ExprWrap,
    def: sem::DefId,
    args: &[ast::syntax::expr::CallArg],
) -> Operand {
    let (params, defaults) = {
        let sig = &ctx.sem.defs.get(def).sig;
        (sig.params.clone(), sig.param_defaults.clone())
    };
    let fields = ctx.sem.defs.get(def).sig.fields.clone();
    let mut slots: Vec<Option<Operand>> = vec![None; params.len()];
    let mut bad = false;
    for (i, arg) in args.iter().enumerate() {
        let idx = match arg.name {
            Some(n) => match params.iter().position(|(s, _)| *s == n) {
                Some(p) => p,
                None => {
                    ctx.diag(e.span, "命名实参无对应构造参数（SemUnknownParam 漏报）");
                    bad = true;
                    continue;
                }
            },
            None => i,
        };
        if idx >= slots.len() || slots[idx].is_some() {
            ctx.diag(e.span, "构造实参槽位冲突（SemArityMismatch 漏报）");
            bad = true;
            continue;
        }
        let v = lower_expr(ctx, &arg.value);
        slots[idx] = Some(coerce(ctx, v, ctx.expr_ty(&arg.value), params[idx].1));
    }
    if !bad {
        for idx in 0..params.len() {
            if slots[idx].is_none() {
                if !defaults.get(idx).copied().unwrap_or(false) {
                    ctx.diag(e.span, "缺少构造实参（SemArityMismatch 漏报）");
                    continue;
                }
                let Some(d) = ctx.find_struct_default(def, idx) else {
                    ctx.diag(e.span, "默认实参 AST 缺失（sem 契约破缺）");
                    continue;
                };
                let v = lower_expr(ctx, d);
                slots[idx] = Some(coerce(ctx, v, ctx.expr_ty(d), params[idx].1));
            }
        }
    }
    let base = ctx.temp();
    for (idx, v) in slots.into_iter().enumerate() {
        let Some(v) = v else { continue };
        let (pname, pty) = params[idx];
        // 仅 val/var 参数有字段槽；仅构造参数求值后丢弃
        if let Some(fidx) = fields.iter().position(|(s, _)| *s == pname) {
            let fty = fields[fidx].1;
            let off = crate::layout::field_slot(ctx, def, fidx);
            copy_slots(ctx, (base, off), (v, 0), pty, fty, e.span);
        }
    }
    Operand::Temp(base)
}

/// struct 构造器调用 P(1, y = 2)：槽位机（位置/命名实参），逐字段 Store
fn struct_ctor(
    ctx: &mut Ctx,
    e: &ExprWrap,
    def: sem::DefId,
    args: &[ast::syntax::expr::CallArg],
) -> Operand {
    // M3 主构造：参数槽机（val/var 参数落字段槽；仅构造参数求值后丢弃）
    if !ctx.sem.defs.get(def).sig.params.is_empty() {
        return primary_ctor_call(ctx, e, def, args);
    }
    let fields = ctx.sem.defs.get(def).sig.fields.clone();
    let mut slots: Vec<Option<Operand>> = vec![None; fields.len()];
    let mut bad = false;
    for (i, arg) in args.iter().enumerate() {
        let idx = match arg.name {
            Some(n) => match fields.iter().position(|(s, _)| *s == n) {
                Some(p) => p,
                None => {
                    ctx.diag(e.span, "命名实参无对应字段（SemUnknownParam 漏报）");
                    bad = true;
                    continue;
                }
            },
            None => i,
        };
        if idx >= slots.len() || slots[idx].is_some() {
            ctx.diag(e.span, "构造实参槽位冲突（SemArityMismatch 漏报）");
            bad = true;
            continue;
        }
        let v = lower_expr(ctx, &arg.value);
        slots[idx] = Some(coerce(ctx, v, ctx.expr_ty(&arg.value), fields[idx].1));
    }
    if !bad {
        for slot in slots.iter() {
            if slot.is_none() {
                ctx.diag(e.span, "缺少构造实参（SemArityMismatch 漏报）");
            }
        }
    }
    let base = ctx.temp();
    for (idx, v) in slots.into_iter().enumerate() {
        if let Some(v) = v {
            let off = crate::layout::field_slot(ctx, def, idx);
            copy_slots(
                ctx,
                (base, off),
                (v, 0),
                fields[idx].1,
                fields[idx].1,
                e.span,
            );
        }
    }
    Operand::Temp(base)
}

/// 枚举变体构造 Opt.Some(x)：tag + payload 槽（实参仅位置形，sem 已拒命名）
fn variant_ctor(
    ctx: &mut Ctx,
    e: &ExprWrap,
    parent: sem::DefId,
    index: usize,
    args: &[ast::syntax::expr::CallArg],
) -> Operand {
    if index > 255 {
        ctx.diag_code(
            DiagCode::IrcTooManyVariants,
            e.span,
            "变体序号超 tag 槽 i8 上限（>255 变体的枚举无法 lowering）",
        );
    }
    let params = ctx.sem.defs.get(parent).sig.variants[index].1.clone();
    if args.len() != params.len() {
        ctx.diag(e.span, "变体实参数不匹配（SemArityMismatch 漏报）");
    }
    let base = ctx.temp();
    ctx.emit(Instr::Store {
        base,
        idx: Operand::Const(Const::Int(0)),
        src: Operand::Const(Const::Int(index as i128)),
        ty: ctx.i8_ty,
    });
    for (i, arg) in args.iter().enumerate() {
        if i >= params.len() {
            break;
        }
        let v = lower_expr(ctx, &arg.value);
        let v = coerce(ctx, v, ctx.expr_ty(&arg.value), params[i].1);
        let off = 1 + crate::layout::variant_param_slot(ctx, parent, index, i);
        copy_slots(ctx, (base, off), (v, 0), params[i].1, params[i].1, e.span);
    }
    Operand::Temp(base)
}

/// 成员名提取（Member 的 name 字段）
fn member_name(e: &ExprWrap) -> ast::interner::Symbol {
    match &e.kind {
        ExprKind::Member { name, .. } => *name,
        _ => ast::interner::Symbol(u32::MAX),
    }
}

/// EnumVariant DefId → 其父 enum DefId
fn def_of_parent(ctx: &mut Ctx, def: sem::DefId) -> sem::DefId {
    match ctx.sem.defs.get(def).kind {
        DefKind::EnumVariant { parent, .. } => parent,
        _ => def,
    }
}

/// 成员访问：E.A（命名空间，expr_def=EnumVariant，不求值 base）；
/// p.x（expr_def=StructField → 槽 Load）。?. 的 nullable 安全调用在 M2b
fn lower_member(ctx: &mut Ctx, e: &ExprWrap, base: &ExprWrap, safe: bool) -> Operand {
    if safe {
        return lower_safe_member(ctx, e, base);
    }
    // 先查 expr_def（sem 写入时直接可用）；缺失则按名本地解析
    let resolved =
        ctx.sem
            .expr_def
            .get(e.id)
            .copied()
            .map(|def| match &ctx.sem.defs.get(def).kind {
                DefKind::EnumVariant { index, .. } => crate::layout::MemberRes::Variant {
                    parent: def_of_parent(ctx, def),
                    index: *index,
                },
                DefKind::StructField { parent, index, .. } => crate::layout::MemberRes::Field {
                    parent: *parent,
                    index: *index,
                },
                _ => crate::layout::MemberRes::Field {
                    parent: def,
                    index: usize::MAX,
                },
            });
    let resolved =
        resolved.or_else(|| crate::layout::resolve_member(ctx, ctx.expr_ty(base), member_name(e)));
    let Some(res) = resolved else {
        ctx.diag(e.span, "成员访问无法解析（sem 契约破缺）");
        return Operand::Const(Const::Unit);
    };
    match res {
        crate::layout::MemberRes::Variant { index, .. } => variant_value(ctx, e, index),
        crate::layout::MemberRes::Field { parent, index } => {
            let bop = lower_expr(ctx, base);
            let fty = ctx.sem.defs.get(parent).sig.fields[index].1;
            if is_class_ty(ctx, ctx.expr_ty(base)) {
                // M4 slice 1：class 字段经对象指针 + 偏移读
                let obj = as_base(ctx, bop, e.span);
                let addr = class_field_addr(ctx, obj, parent, index);
                let v = ctx.temp();
                ctx.emit(Instr::LoadPtr {
                    dst: v,
                    ptr: addr,
                    ty: fty,
                });
                return Operand::Temp(v);
            }
            let b = as_base(ctx, bop, e.span);
            let slot = crate::layout::field_slot(ctx, parent, index);
            read_slots(ctx, b, slot, fty, e.span)
        }
    }
}

/// 常量元组下标 → (元素类型, 值内部槽偏移)。越界/非字面量均为契约破缺。
fn tuple_index_slot(
    ctx: &mut Ctx,
    bty: TyId,
    const_index: Option<usize>,
    span: Span,
) -> Option<(TyId, usize)> {
    let TyKind::Tuple(ts) = ctx.sem.tys.kind(bty) else {
        return None;
    };
    let Some(i) = const_index else {
        ctx.diag(span, "元组下标非整数字面量（sem 契约破缺）");
        return None;
    };
    let Some(ety) = ts.get(i).copied() else {
        ctx.diag(span, "元组下标越界（sem 契约破缺）");
        return None;
    };
    let off = ts[..i]
        .iter()
        .map(|t| crate::layout::scalar_slots(ctx, *t))
        .sum();
    Some((ety, off))
}

/// 索引访问：数组下标动态寻址（i × 元素槽数）；元组下标是常量槽偏移。
fn lower_index(
    ctx: &mut Ctx,
    e: &ExprWrap,
    base: &ExprWrap,
    index: &ExprWrap,
    const_index: Option<usize>,
) -> Operand {
    let bty = ctx.expr_ty(base);
    let bop = lower_expr(ctx, base);
    let b = as_base(ctx, bop, e.span);
    if let Some((elem, off)) = tuple_index_slot(ctx, bty, const_index, e.span) {
        return read_slots(ctx, b, off, elem, e.span);
    }
    let TyKind::Array(elem, _) = ctx.sem.tys.kind(bty) else {
        ctx.diag(e.span, "非数组/元组索引的 lowering 未支持（sem 契约破缺）");
        return Operand::Const(Const::Unit);
    };
    let elem = *elem;
    let mut idx = lower_expr(ctx, index);
    let es = crate::layout::scalar_slots(ctx, elem);
    if es > 1 {
        let ity = ctx.expr_ty(index);
        let dst = ctx.temp();
        ctx.emit(Instr::Bin {
            dst,
            op: BinOp::Mul,
            l: idx,
            r: Operand::Const(Const::Int(es as i128)),
            ty: ity,
        });
        idx = Operand::Temp(dst);
    }
    let dst = ctx.temp();
    if es <= 1 || !crate::layout::is_aggregate(ctx, elem) {
        // S3：标量元素（含多槽 i128）单 Load——llvme split/join lo/hi
        ctx.emit(Instr::Load {
            dst,
            base: b,
            idx,
            ty: elem,
        });
    } else {
        // 多槽元素：动态基址下标，逐槽 Load/Store 攒新基址
        //（copy_slots 只支持常量槽偏移，动态形态在本处展开）
        for sld in 0..es {
            let sty = crate::layout::slot_ty(ctx, elem, sld);
            let t = ctx.temp();
            let off = if sld == 0 {
                idx.clone()
            } else {
                let o = ctx.temp();
                ctx.emit(Instr::Bin {
                    dst: o,
                    op: BinOp::Add,
                    l: idx.clone(),
                    r: Operand::Const(Const::Int(sld as i128)),
                    ty: ctx.expr_ty(index),
                });
                Operand::Temp(o)
            };
            ctx.emit(Instr::Load {
                dst: t,
                base: b,
                idx: off,
                ty: sty,
            });
            ctx.emit(Instr::Store {
                base: dst,
                idx: Operand::Const(Const::Int(sld as i128)),
                src: Operand::Temp(t),
                ty: sty,
            });
        }
    }
    Operand::Temp(dst)
}

/// 数组字面量 [1, 2, 3]：基址 + 逐元素槽 Store
fn array_literal(ctx: &mut Ctx, e: &ExprWrap, elems: &[ExprWrap]) -> Operand {
    let ty = ctx.expr_ty(e);
    let elem = match ctx.sem.tys.kind(ty) {
        TyKind::Array(el, _) => *el,
        _ => {
            ctx.diag(e.span, "数组字面量类型非 Array（sem 契约破缺）");
            return Operand::Const(Const::Unit);
        }
    };
    let es = crate::layout::scalar_slots(ctx, elem);
    let base = ctx.temp();
    let mut off = 0;
    for el in elems {
        let v = lower_expr(ctx, el);
        let v = coerce(ctx, v, ctx.expr_ty(el), elem);
        copy_slots(ctx, (base, off), (v, 0), elem, elem, e.span);
        off += es;
    }
    Operand::Temp(base)
}

/// 元组字面量 (a, b)：槽序即元素序
fn tuple_literal(ctx: &mut Ctx, e: &ExprWrap, elems: &[ExprWrap]) -> Operand {
    let ty = ctx.expr_ty(e);
    let elem_tys: Vec<TyId> = match ctx.sem.tys.kind(ty) {
        TyKind::Tuple(es) => es.clone(),
        _ => {
            ctx.diag(e.span, "元组字面量类型非 Tuple（sem 契约破缺）");
            return Operand::Const(Const::Unit);
        }
    };
    let base = ctx.temp();
    let mut off = 0;
    for (i, el) in elems.iter().enumerate() {
        let ety = elem_tys.get(i).copied().unwrap_or(ty);
        let v = lower_expr(ctx, el);
        let v = coerce(ctx, v, ctx.expr_ty(el), ety);
        copy_slots(ctx, (base, off), (v, 0), ety, ety, e.span);
        off += crate::layout::scalar_slots(ctx, ety);
    }
    Operand::Temp(base)
}

/// 数组填充 [v; n]：值一次求值 + 下标循环（n 为 AST 常量；循环形态
/// 避免按 count 展开指令，count 任意大都恒定体积）
fn array_repeat(ctx: &mut Ctx, e: &ExprWrap, value: &ExprWrap, count: u32) -> Operand {
    let ty = ctx.expr_ty(e);
    let elem = match ctx.sem.tys.kind(ty) {
        TyKind::Array(el, _) => *el,
        _ => {
            ctx.diag(e.span, "数组填充类型非 Array（sem 契约破缺）");
            return Operand::Const(Const::Unit);
        }
    };
    let es = crate::layout::scalar_slots(ctx, elem);
    let v = lower_expr(ctx, value);
    let v = coerce(ctx, v, ctx.expr_ty(value), elem);
    let base = ctx.temp();
    // 动态槽写入的静态尺寸声明（llvme 分配存储的信息源）
    ctx.emit(Instr::Slot {
        dst: base,
        count: (es.max(1) * count as usize) as u32,
    });
    let ity = ctx.sem.tys.default_int;
    // 元素序计数器（slot）
    let counter = ctx.temp();
    ctx.emit(Instr::Store {
        base: counter,
        idx: Operand::Const(Const::Int(0)),
        src: Operand::Const(Const::Int(0)),
        ty: ity,
    });
    let cond_b = ctx.reserve_block();
    let body_b = ctx.reserve_block();
    let done_b = ctx.reserve_block();
    ctx.terminate(Term::Jmp(cond_b));

    ctx.switch_to(cond_b);
    let c = ctx.temp();
    ctx.emit(Instr::Load {
        dst: c,
        base: counter,
        idx: Operand::Const(Const::Int(0)),
        ty: ity,
    });
    let cmp = ctx.temp();
    ctx.emit(Instr::Bin {
        dst: cmp,
        op: BinOp::Lt,
        l: Operand::Temp(c),
        r: Operand::Const(Const::Int(count as i128)),
        ty: ity,
    });
    ctx.terminate(Term::Br {
        cond: Operand::Temp(cmp),
        then_: body_b,
        else_: done_b,
    });

    ctx.switch_to(body_b);
    let i = ctx.temp();
    ctx.emit(Instr::Load {
        dst: i,
        base: counter,
        idx: Operand::Const(Const::Int(0)),
        ty: ity,
    });
    if es <= 1 {
        ctx.emit(Instr::Store {
            base,
            idx: Operand::Temp(i),
            src: v,
            ty: elem,
        });
    } else {
        let vb = as_base(ctx, v, e.span);
        for s in 0..es {
            let sty = crate::layout::slot_ty(ctx, elem, s);
            let t = ctx.temp();
            ctx.emit(Instr::Load {
                dst: t,
                base: vb,
                idx: Operand::Const(Const::Int(s as i128)),
                ty: sty,
            });
            let off = if s == 0 {
                Operand::Temp(i)
            } else {
                let o = ctx.temp();
                ctx.emit(Instr::Bin {
                    dst: o,
                    op: BinOp::Add,
                    l: Operand::Temp(i),
                    r: Operand::Const(Const::Int(s as i128)),
                    ty: ity,
                });
                Operand::Temp(o)
            };
            ctx.emit(Instr::Store {
                base,
                idx: off,
                src: Operand::Temp(t),
                ty: sty,
            });
        }
    }
    let one = ctx.temp();
    ctx.emit(Instr::Bin {
        dst: one,
        op: BinOp::Add,
        l: Operand::Temp(i),
        r: Operand::Const(Const::Int(1)),
        ty: ity,
    });
    ctx.emit(Instr::Store {
        base: counter,
        idx: Operand::Const(Const::Int(0)),
        src: Operand::Temp(one),
        ty: ity,
    });
    ctx.terminate(Term::Jmp(cond_b));

    ctx.switch_to(done_b);
    Operand::Temp(base)
}

/// as?：nullable → nullable 运行时 flag 分支拷贝；非 nullable 源恒非空
/// （flag=1 + payload 转换拷贝）
fn nullable_cast(ctx: &mut Ctx, e: &ExprWrap, inner: &ExprWrap) -> Operand {
    let to = ctx.expr_ty(e);
    let u_inner = match ctx.sem.tys.kind(to) {
        TyKind::Nullable(inner) => *inner,
        // sem 若给非 nullable 结果型（如恒非空推断），退化为数值转换
        _ => {
            let from = ctx.expr_ty(inner);
            let op = lower_expr(ctx, inner);
            return coerce(ctx, op, from, to);
        }
    };
    let from = ctx.expr_ty(inner);
    let op = lower_expr(ctx, inner);
    let base = ctx.temp();
    match ctx.sem.tys.kind(from) {
        TyKind::Nullable(_) => {
            let sb = as_base(ctx, op, e.span);
            let flag = ctx.temp();
            ctx.emit(Instr::Load {
                dst: flag,
                base: sb,
                idx: Operand::Const(Const::Int(0)),
                ty: ctx.i8_ty,
            });
            let nonnull_b = ctx.reserve_block();
            let null_b = ctx.reserve_block();
            let merge_b = ctx.reserve_block();
            ctx.terminate(Term::Br {
                cond: Operand::Temp(flag),
                then_: nonnull_b,
                else_: null_b,
            });
            ctx.switch_to(nonnull_b);
            ctx.emit(Instr::Store {
                base,
                idx: Operand::Const(Const::Int(0)),
                src: Operand::Const(Const::Int(1)),
                ty: ctx.i8_ty,
            });
            // 只拷 payload：flag 已在上文各分支单独写。类型取两侧
            // nullable 的 payload 型（值内部下标 0..，与 off=1 无关）
            let f_inner = match ctx.sem.tys.kind(from) {
                TyKind::Nullable(i) => *i,
                _ => unreachable!("外层 match 已判定 Nullable"),
            };
            copy_slots(
                ctx,
                (base, 1),
                (Operand::Temp(sb), 1),
                f_inner,
                u_inner,
                e.span,
            );
            ctx.terminate(Term::Jmp(merge_b));
            ctx.switch_to(null_b);
            ctx.emit(Instr::Store {
                base,
                idx: Operand::Const(Const::Int(0)),
                src: Operand::Const(Const::Int(0)),
                ty: ctx.i8_ty,
            });
            ctx.terminate(Term::Jmp(merge_b));
            ctx.switch_to(merge_b);
            Operand::Temp(base)
        }
        _ => {
            ctx.emit(Instr::Store {
                base,
                idx: Operand::Const(Const::Int(0)),
                src: Operand::Const(Const::Int(1)),
                ty: ctx.i8_ty,
            });
            copy_slots(ctx, (base, 1), (op, 0), from, u_inner, e.span);
            Operand::Temp(base)
        }
    }
}

/// as! / !!：拆 nullable——flag 检查，null 走 unreachable（sem 已担保
/// 非空的契约破缺路径），payload 逐槽转换为目标型
fn unwrap(ctx: &mut Ctx, e: &ExprWrap, inner: &ExprWrap, to: TyId) -> Operand {
    let from = ctx.expr_ty(inner);
    let op = lower_expr(ctx, inner);
    let s_inner = match ctx.sem.tys.kind(from) {
        TyKind::Nullable(inner) => *inner,
        // 对非 nullable 源拆包 = 纯转换
        _ => return coerce(ctx, op, from, to),
    };
    let sb = as_base(ctx, op, e.span);
    let flag = ctx.temp();
    ctx.emit(Instr::Load {
        dst: flag,
        base: sb,
        idx: Operand::Const(Const::Int(0)),
        ty: ctx.i8_ty,
    });
    let ok_b = ctx.reserve_block();
    let bad_b = ctx.reserve_block();
    ctx.terminate(Term::Br {
        cond: Operand::Temp(flag),
        then_: ok_b,
        else_: bad_b,
    });
    ctx.switch_to(bad_b);
    ctx.terminate(Term::Unreachable);
    ctx.switch_to(ok_b);
    if !crate::layout::is_aggregate(ctx, to) {
        let sty = crate::layout::slot_ty(ctx, from, 1);
        let t = ctx.temp();
        ctx.emit(Instr::Load {
            dst: t,
            base: sb,
            idx: Operand::Const(Const::Int(1)),
            ty: sty,
        });
        coerce(ctx, Operand::Temp(t), sty, to)
    } else {
        let base = ctx.temp();
        copy_slots(ctx, (base, 0), (Operand::Temp(sb), 1), s_inner, to, e.span);
        Operand::Temp(base)
    }
}

// ---- 模式匹配 lowering（M2b：is / when / for / val 条件共用）----

/// 收集 Or 各替代共享绑定的名字（sem 已保证同名同变量）。
fn collect_pattern_bindings(
    pat: &ast::syntax::pat::PatWrap,
    names: &mut Vec<ast::interner::Symbol>,
) {
    match &pat.kind {
        ast::syntax::pat::PatKind::Binding { name, .. } => {
            if !names.contains(name) {
                names.push(*name);
            }
        }
        ast::syntax::pat::PatKind::Or(ps) => {
            for p in ps {
                collect_pattern_bindings(p, names);
            }
        }
        ast::syntax::pat::PatKind::Tuple { elements, .. }
        | ast::syntax::pat::PatKind::Array { elements, .. } => {
            for p in elements {
                collect_pattern_bindings(p, names);
            }
        }
        ast::syntax::pat::PatKind::Constructor { args, .. } => {
            for p in args {
                collect_pattern_bindings(p, names);
            }
        }
        ast::syntax::pat::PatKind::Struct { fields, .. } => {
            for (_, p) in fields {
                collect_pattern_bindings(p, names);
            }
        }
        _ => {}
    }
}

/// 解构声明是 `var` 时，把模式刚写入 env 的 Temp 绑定迁到 Slot，
/// 使后续赋值写穿透到原存储（sem 已给这些 Def 标 mutable）。
pub(crate) fn make_pattern_bindings_mutable(ctx: &mut Ctx, pat: &ast::syntax::pat::PatWrap) {
    let mut names = Vec::new();
    collect_pattern_bindings(pat, &mut names);
    for name in names {
        let Some((Place::Temp(t), ty)) = ctx.lookup(name) else {
            continue;
        };
        let slot = ctx.temp();
        copy_slots(ctx, (slot, 0), (Operand::Temp(t), 0), ty, ty, pat.span);
        ctx.env_top().insert(name, (Place::Slot(slot), ty));
    }
}

/// 含绑定的 Or：每替代独立条件块；命中后把该替代产生的绑定写入
/// 共享槽，汇合后再读出为普通 Temp——避免 phi，绑定值跨替代一致。
fn lower_or_pattern(
    ctx: &mut Ctx,
    ps: &[ast::syntax::pat::PatWrap],
    subject: Operand,
    subject_ty: TyId,
    span: Span,
) -> Operand {
    let mut names = Vec::new();
    for p in ps {
        collect_pattern_bindings(p, &mut names);
    }
    if names.is_empty() {
        let mut cond = lower_pattern(ctx, &ps[0], subject.clone(), subject_ty, span);
        for p in &ps[1..] {
            let c = lower_pattern(ctx, p, subject.clone(), subject_ty, span);
            cond = or_operands(ctx, cond, c);
        }
        return cond;
    }

    let bool_ty = ctx.sem.tys.bool_;
    let result = ctx.temp();
    ctx.emit(Instr::Copy {
        dst: result,
        src: Operand::Const(Const::Bool(false)),
        ty: bool_ty,
    });
    let merge = ctx.reserve_block();
    let mut shared: Vec<(ast::interner::Symbol, TempId, Option<TyId>)> = names
        .into_iter()
        .map(|name| (name, ctx.temp(), None))
        .collect();

    for p in ps {
        let bind_b = ctx.reserve_block();
        let fail_b = ctx.reserve_block();
        let c = lower_pattern(ctx, p, subject.clone(), subject_ty, p.span);
        ctx.terminate(Term::Br {
            cond: c,
            then_: bind_b,
            else_: fail_b,
        });
        ctx.switch_to(bind_b);
        for (name, slot, ty) in &mut shared {
            let Some((place, pty)) = ctx.lookup(*name) else {
                ctx.diag(p.span, "Or 替代绑定名不一致（sem 契约破缺）");
                continue;
            };
            if let Some(prev) = ty
                && *prev != pty
            {
                ctx.diag(p.span, "Or 替代绑定类型不一致（sem 契约破缺）");
            }
            *ty = Some(pty);
            let v = read_place(ctx, place, pty, p.span);
            copy_slots(ctx, (*slot, 0), (v, 0), pty, pty, p.span);
        }
        ctx.emit(Instr::Copy {
            dst: result,
            src: Operand::Const(Const::Bool(true)),
            ty: bool_ty,
        });
        ctx.terminate(Term::Jmp(merge));
        ctx.switch_to(fail_b);
    }
    ctx.emit(Instr::Copy {
        dst: result,
        src: Operand::Const(Const::Bool(false)),
        ty: bool_ty,
    });
    ctx.terminate(Term::Jmp(merge));
    ctx.switch_to(merge);
    for (name, slot, ty) in shared {
        let ty = ty.unwrap_or(ctx.sem.tys.unit);
        let v = read_place(ctx, Place::Slot(slot), ty, span);
        let t = as_base(ctx, v, span);
        ctx.env_top().insert(name, (Place::Temp(t), ty));
    }
    Operand::Temp(result)
}

/// 模式匹配 lowering：返回 bool 条件操作数；绑定写入当前 env 作用域
/// （调用方负责作用域的推入/弹出时机）。
/// subject：待匹配值（标量操作数或聚合基址）；subject_ty 其类型。
pub(crate) fn lower_pattern(
    ctx: &mut Ctx,
    pat: &ast::syntax::pat::PatWrap,
    subject: Operand,
    subject_ty: TyId,
    span: Span,
) -> Operand {
    use ast::syntax::pat::PatKind;
    match &pat.kind {
        PatKind::Binding { name, ty, .. } => {
            let bty = ty
                .as_ref()
                .and_then(|t| ctx.sem.ty_of_node.get(t.id).copied())
                .unwrap_or(subject_ty);
            let v = coerce(ctx, subject, subject_ty, bty);
            let t = as_base(ctx, v, pat.span);
            ctx.env_top().insert(*name, (Place::Temp(t), bty));
            Operand::Const(Const::Bool(true))
        }
        PatKind::Wildcard => Operand::Const(Const::Bool(true)),
        PatKind::Bool(b) => {
            let ty = ctx.sem.tys.bool_;
            let s = coerce(ctx, subject, subject_ty, ty);
            cmp_const(ctx, BinOp::Eq, s, Const::Bool(*b), ty)
        }
        PatKind::Literal(kind) => {
            let lit = literal(ctx, *kind, pat.span);
            let ty = match kind {
                LiteralKind::Integer => ctx.sem.tys.default_int,
                LiteralKind::Float => ctx.sem.tys.default_float,
                LiteralKind::String => ctx.sem.tys.string,
                LiteralKind::Char => ctx.sem.tys.char_,
            };
            let s = coerce(ctx, subject, subject_ty, ty);
            if matches!(kind, LiteralKind::String) {
                string_eq(ctx, s, lit)
            } else {
                cmp_op(ctx, BinOp::Eq, s, lit, ty)
            }
        }
        PatKind::Null => match ctx.sem.tys.kind(subject_ty) {
            TyKind::Nullable(_) => {
                let b = as_base(ctx, subject, span);
                let flag = ctx.temp();
                ctx.emit(Instr::Load {
                    dst: flag,
                    base: b,
                    idx: Operand::Const(Const::Int(0)),
                    ty: ctx.i8_ty,
                });
                let t = ctx.temp();
                ctx.emit(Instr::Un {
                    dst: t,
                    op: UnOp::Not,
                    a: Operand::Temp(flag),
                    ty: ctx.sem.tys.bool_,
                });
                Operand::Temp(t)
            }
            _ => Operand::Const(Const::Bool(true)),
        },
        PatKind::Constructor { args, .. } => {
            // 变体序号：pat_res 优先（sem resolve 写入），缺失按主语类型+名本地解析
            let (parent, vidx) = match ctx.sem.pat_res.get(pat.id).copied() {
                Some(pr) => (pr.ctor, pr.variant.unwrap_or(0)),
                None => {
                    ctx.diag(pat.span, "构造器模式无 pat_res（sem 契约破缺）");
                    return Operand::Const(Const::Bool(false));
                }
            };
            let parent = match ctx.sem.tys.kind(subject_ty) {
                TyKind::Named(d, _) => *d,
                TyKind::Nullable(_) => {
                    ctx.diag(pat.span, "nullable 主语的构造器模式在 M-later");
                    return Operand::Const(Const::Bool(false));
                }
                _ => parent,
            };
            let b = as_base(ctx, subject, span);
            let tag = ctx.temp();
            ctx.emit(Instr::Load {
                dst: tag,
                base: b,
                idx: Operand::Const(Const::Int(0)),
                ty: ctx.i8_ty,
            });
            let mut cond = cmp_const(
                ctx,
                BinOp::Eq,
                Operand::Temp(tag),
                Const::Int(vidx as i128),
                ctx.i8_ty,
            );
            // payload 实参绑定（Load + 递归； conjunction 经 && 物化）
            let params = ctx.sem.defs.get(parent).sig.variants[vidx].1.clone();
            for (i, arg) in args.iter().enumerate() {
                let Some((_, pty)) = params.get(i) else {
                    ctx.diag(arg.span, "变体模式实参越界（sem 契约破缺）");
                    break;
                };
                let off = 1 + crate::layout::variant_param_slot(ctx, parent, vidx, i);
                let v = ctx.temp();
                let sty = crate::layout::slot_ty(ctx, subject_ty, off);
                ctx.emit(Instr::Load {
                    dst: v,
                    base: b,
                    idx: Operand::Const(Const::Int(off as i128)),
                    ty: sty,
                });
                let c = lower_pattern(ctx, arg, Operand::Temp(v), *pty, arg.span);
                cond = and_operands(ctx, cond, c);
            }
            cond
        }
        PatKind::Struct { fields, .. } => {
            let parent = match ctx.sem.tys.kind(subject_ty) {
                TyKind::Named(d, _) => *d,
                _ => {
                    ctx.diag(pat.span, "Struct 模式主语非命名类型（sem 契约破缺）");
                    return Operand::Const(Const::Bool(false));
                }
            };
            let b = as_base(ctx, subject, span);
            let mut cond = Operand::Const(Const::Bool(true));
            for (fname, fpat) in fields {
                let Some(idx) = ctx
                    .sem
                    .defs
                    .get(parent)
                    .sig
                    .fields
                    .iter()
                    .position(|(n, _)| n == fname)
                else {
                    ctx.diag(fpat.span, "模式字段名不存在（sem 契约破缺）");
                    continue;
                };
                let fty = ctx.sem.defs.get(parent).sig.fields[idx].1;
                let off = crate::layout::field_slot(ctx, parent, idx);
                let v = ctx.temp();
                let sty = crate::layout::slot_ty(ctx, subject_ty, off);
                ctx.emit(Instr::Load {
                    dst: v,
                    base: b,
                    idx: Operand::Const(Const::Int(off as i128)),
                    ty: sty,
                });
                let c = lower_pattern(ctx, fpat, Operand::Temp(v), fty, fpat.span);
                cond = and_operands(ctx, cond, c);
            }
            cond
        }
        PatKind::Tuple { elements, rest } | PatKind::Array { elements, rest } => {
            let elem_tys: Vec<TyId> = match ctx.sem.tys.kind(subject_ty) {
                TyKind::Tuple(es) => es.clone(),
                TyKind::Array(el, n) => vec![*el; *n as usize],
                _ => {
                    ctx.diag(pat.span, "元组/数组模式主语类型不匹配（sem 契约破缺）");
                    return Operand::Const(Const::Bool(false));
                }
            };
            let b = as_base(ctx, subject, span);
            let mut cond = Operand::Const(Const::Bool(true));
            for (i, el) in elements.iter().enumerate() {
                let pos = match rest {
                    Some(r) if i >= *r => elem_tys.len().saturating_sub(elements.len() - i),
                    _ => i,
                };
                let Some(ety) = elem_tys.get(pos).copied() else {
                    ctx.diag(el.span, "模式元素越界（sem 契约破缺）");
                    return Operand::Const(Const::Bool(false));
                };
                let es = crate::layout::scalar_slots(ctx, ety);
                if es > 1 {
                    ctx.diag(el.span, "聚合元素的元组/数组模式在 M-later");
                    return Operand::Const(Const::Bool(false));
                }
                let v = ctx.temp();
                ctx.emit(Instr::Load {
                    dst: v,
                    base: b,
                    idx: Operand::Const(Const::Int(pos as i128)),
                    ty: ety,
                });
                let c = lower_pattern(ctx, el, Operand::Temp(v), ety, el.span);
                cond = and_operands(ctx, cond, c);
            }
            cond
        }
        PatKind::Or(ps) => lower_or_pattern(ctx, ps, subject, subject_ty, span),
        PatKind::Range { lo, hi, inclusive } => {
            // 区间模式：lo <= subject <= hi（..= 含上界，.. 不含）
            let ity = subject_ty;
            let lov = pattern_bound(ctx, lo);
            let c1 = cmp_const(ctx, BinOp::Ge, subject.clone(), lov, ity);
            let hv = pattern_bound(ctx, hi);
            if *inclusive {
                let c2 = cmp_const(ctx, BinOp::Le, subject, hv, ity);
                and_operands(ctx, c1, c2)
            } else {
                let c2 = cmp_const(ctx, BinOp::Lt, subject, hv, ity);
                and_operands(ctx, c1, c2)
            }
        }
        PatKind::Error(_) | PatKind::Missing(_) => {
            ctx.diag(pat.span, "错误/缺失模式穿透 sem 门禁（内部不变量破缺）");
            Operand::Const(Const::Bool(false))
        }
    }
}

/// 字面量/布尔模式的常量操作数
/// 模式边界的字面量求值（Literal/Bool 模式的 lo/hi）
fn pattern_bound(ctx: &mut Ctx, pat: &ast::syntax::pat::PatWrap) -> Const {
    match &pat.kind {
        ast::syntax::pat::PatKind::Literal(kind) => {
            let text = pat.span.slice(ctx.source);
            match kind {
                LiteralKind::Integer => text
                    .replace('_', "")
                    .parse()
                    .map(Const::Int)
                    .unwrap_or(Const::Int(0)),
                LiteralKind::Float => text
                    .replace('_', "")
                    .parse()
                    .map(Const::Float)
                    .unwrap_or(Const::Float(0.0)),
                _ => Const::Int(0),
            }
        }
        ast::syntax::pat::PatKind::Bool(b) => Const::Bool(*b),
        _ => Const::Int(0),
    }
}

/// 物化 Range 的上界判定：标记位为 1 用 `<=`，否则 `<`。
pub(crate) fn range_upper(
    ctx: &mut Ctx,
    s: Operand,
    hi: Operand,
    flag: Operand,
    ty: TyId,
) -> Operand {
    let result = ctx.temp();
    let le_b = ctx.reserve_block();
    let lt_b = ctx.reserve_block();
    let done = ctx.reserve_block();
    let closed = {
        let t = ctx.temp();
        ctx.emit(Instr::Bin {
            dst: t,
            op: BinOp::Ne,
            l: flag,
            r: Operand::Const(Const::Int(0)),
            ty: ctx.i8_ty,
        });
        Operand::Temp(t)
    };
    ctx.terminate(Term::Br {
        cond: closed,
        then_: le_b,
        else_: lt_b,
    });
    ctx.switch_to(le_b);
    let c1 = cmp_op(ctx, BinOp::Le, s.clone(), hi.clone(), ty);
    ctx.emit(Instr::Copy {
        dst: result,
        src: c1,
        ty: ctx.sem.tys.bool_,
    });
    ctx.terminate(Term::Jmp(done));
    ctx.switch_to(lt_b);
    let c2 = cmp_op(ctx, BinOp::Lt, s, hi, ty);
    ctx.emit(Instr::Copy {
        dst: result,
        src: c2,
        ty: ctx.sem.tys.bool_,
    });
    ctx.terminate(Term::Jmp(done));
    ctx.switch_to(done);
    Operand::Temp(result)
}

/// subject OP 操作数比较（操作数公共型 = ty 参数）
fn cmp_op(ctx: &mut Ctx, op: BinOp, s: Operand, r: Operand, ty: TyId) -> Operand {
    let dst = ctx.temp();
    ctx.emit(Instr::Bin {
        dst,
        op,
        l: s,
        r,
        ty,
    });
    Operand::Temp(dst)
}

/// subject OP const 比较（操作数公共型 = ty 参数）
fn cmp_const(ctx: &mut Ctx, op: BinOp, s: Operand, c: Const, ty: TyId) -> Operand {
    cmp_op(ctx, op, s, Operand::Const(c), ty)
}

/// 两个 bool 操作数的 && 物化（Br 链）
fn and_operands(ctx: &mut Ctx, l: Operand, r: Operand) -> Operand {
    // 常量短路（发射期折叠，与 coerce 常量快路径同哲学）
    match (&l, &r) {
        (Operand::Const(Const::Bool(true)), _) => return r,
        (Operand::Const(Const::Bool(false)), _) => return l,
        (_, Operand::Const(Const::Bool(true))) => return l,
        (_, Operand::Const(Const::Bool(false))) => return r,
        _ => {}
    }
    materialize_logic_operands(ctx, l, r, true)
}

/// 两个 bool 操作数的 || 物化
fn or_operands(ctx: &mut Ctx, l: Operand, r: Operand) -> Operand {
    match (&l, &r) {
        (Operand::Const(Const::Bool(false)), _) => return r,
        (Operand::Const(Const::Bool(true)), _) => return l,
        (_, Operand::Const(Const::Bool(false))) => return l,
        (_, Operand::Const(Const::Bool(true))) => return r,
        _ => {}
    }
    materialize_logic_operands(ctx, l, r, false)
}

fn materialize_logic_operands(ctx: &mut Ctx, l: Operand, r: Operand, is_and: bool) -> Operand {
    let result = ctx.temp();
    let bool_ty = ctx.sem.tys.bool_;
    let eval_rhs = ctx.reserve_block();
    let done_true = ctx.reserve_block();
    let done_false = ctx.reserve_block();
    let merge = ctx.reserve_block();
    let (then_, else_) = if is_and {
        (eval_rhs, done_false)
    } else {
        (done_true, eval_rhs)
    };
    ctx.terminate(Term::Br {
        cond: l,
        then_,
        else_,
    });
    ctx.switch_to(eval_rhs);
    ctx.terminate(Term::Br {
        cond: r,
        then_: done_true,
        else_: done_false,
    });
    ctx.switch_to(done_true);
    ctx.emit(Instr::Copy {
        dst: result,
        src: Operand::Const(Const::Bool(true)),
        ty: bool_ty,
    });
    ctx.terminate(Term::Jmp(merge));
    ctx.switch_to(done_false);
    ctx.emit(Instr::Copy {
        dst: result,
        src: Operand::Const(Const::Bool(false)),
        ty: bool_ty,
    });
    ctx.terminate(Term::Jmp(merge));
    ctx.switch_to(merge);
    Operand::Temp(result)
}

/// is：模式匹配 + 绑定进当前作用域；negated 取反。调用方（if/while）
/// 负责在 then 分支作用域内推入/弹出（见 lower_cond_expr）
fn lower_is(
    ctx: &mut Ctx,
    e: &ExprWrap,
    lhs: &ExprWrap,
    pattern: &ast::syntax::pat::PatWrap,
    negated: bool,
) -> Operand {
    let lty = ctx.expr_ty(lhs);
    let s = lower_expr(ctx, lhs);
    let cond = lower_pattern(ctx, pattern, s, lty, e.span);
    if negated {
        let t = ctx.temp();
        ctx.emit(Instr::Un {
            dst: t,
            op: UnOp::Not,
            a: cond,
            ty: ctx.sem.tys.bool_,
        });
        Operand::Temp(t)
    } else {
        cond
    }
}

/// in：区间 → 双比较短路；数组 → 线性扫描循环
fn lower_in(ctx: &mut Ctx, e: &ExprWrap, lhs: &ExprWrap, rhs: &ExprWrap) -> Operand {
    let lty = ctx.expr_ty(lhs);
    let rty = ctx.expr_ty(rhs);
    let s = lower_expr(ctx, lhs);
    match ctx.sem.tys.kind(rty).clone() {
        TyKind::Range(_) => {
            // 区间字面量 1..5（Binary DotDot/DotDotEq）直取 lo/hi；
            // 物化 Range 值按闭区间（Range 类型不区分开闭）
            if let ExprKind::Binary {
                op,
                lhs: lo,
                rhs: hi,
            } = &rhs.kind
                && matches!(op, OperatorKind::DotDot | OperatorKind::DotDotEq)
            {
                let lo_op = lower_expr(ctx, lo);
                let lov = coerce(ctx, lo_op, ctx.expr_ty(lo), lty);
                let hi_op = lower_expr(ctx, hi);
                let hiv = coerce(ctx, hi_op, ctx.expr_ty(hi), lty);
                let c1 = cmp_op(ctx, BinOp::Ge, s.clone(), lov, lty);
                let op2 = if *op == OperatorKind::DotDot {
                    BinOp::Lt
                } else {
                    BinOp::Le
                };
                let c2 = cmp_op(ctx, op2, s, hiv, lty);
                return and_operands(ctx, c1, c2);
            }
            let rhs_op = lower_expr(ctx, rhs);
            let rb = as_base(ctx, rhs_op, e.span);
            let n = crate::layout::scalar_slots(ctx, lty).max(1);
            let lo = ctx.temp();
            ctx.emit(Instr::Load {
                dst: lo,
                base: rb,
                idx: Operand::Const(Const::Int(0)),
                ty: lty,
            });
            let hi = ctx.temp();
            ctx.emit(Instr::Load {
                dst: hi,
                base: rb,
                idx: Operand::Const(Const::Int(n as i128)),
                ty: lty,
            });
            let flag = ctx.temp();
            ctx.emit(Instr::Load {
                dst: flag,
                base: rb,
                idx: Operand::Const(Const::Int((2 * n) as i128)),
                ty: ctx.i8_ty,
            });
            let c1 = cmp_op(ctx, BinOp::Ge, s.clone(), Operand::Temp(lo), lty);
            let c2 = range_upper(ctx, s, Operand::Temp(hi), Operand::Temp(flag), lty);
            and_operands(ctx, c1, c2)
        }
        TyKind::Array(elem, len) => {
            let es = crate::layout::scalar_slots(ctx, elem);
            let rhs_op = lower_expr(ctx, rhs);
            let arr = as_base(ctx, rhs_op, e.span);
            let cty = ctx.sem.tys.default_int;
            let counter = ctx.temp();
            ctx.emit(Instr::Store {
                base: counter,
                idx: Operand::Const(Const::Int(0)),
                src: Operand::Const(Const::Int(0)),
                ty: cty,
            });
            let result = ctx.temp();
            ctx.emit(Instr::Copy {
                dst: result,
                src: Operand::Const(Const::Bool(false)),
                ty: ctx.sem.tys.bool_,
            });
            let cond_b = ctx.reserve_block();
            let body_b = ctx.reserve_block();
            let found_b = ctx.reserve_block();
            let step_b = ctx.reserve_block();
            let done_b = ctx.reserve_block();
            ctx.terminate(Term::Jmp(cond_b));
            ctx.switch_to(cond_b);
            let c = ctx.temp();
            ctx.emit(Instr::Load {
                dst: c,
                base: counter,
                idx: Operand::Const(Const::Int(0)),
                ty: cty,
            });
            let cmp = ctx.temp();
            ctx.emit(Instr::Bin {
                dst: cmp,
                op: BinOp::Lt,
                l: Operand::Temp(c),
                r: Operand::Const(Const::Int(len as i128)),
                ty: cty,
            });
            ctx.terminate(Term::Br {
                cond: Operand::Temp(cmp),
                then_: body_b,
                else_: done_b,
            });
            ctx.switch_to(body_b);
            let eq = if es <= 1 {
                let el = ctx.temp();
                ctx.emit(Instr::Load {
                    dst: el,
                    base: arr,
                    idx: Operand::Temp(c),
                    ty: elem,
                });
                let s2 = coerce(ctx, s.clone(), lty, elem);
                let el_op = Operand::Temp(el);
                structural_eq(ctx, s2, el_op, elem)
            } else {
                let sb = as_base(ctx, s.clone(), e.span);
                let base_idx = ctx.temp();
                ctx.emit(Instr::Bin {
                    dst: base_idx,
                    op: BinOp::Mul,
                    l: Operand::Temp(c),
                    r: Operand::Const(Const::Int(es as i128)),
                    ty: cty,
                });
                let mut cond = Operand::Const(Const::Bool(true));
                for sld in 0..es {
                    let sty = crate::layout::slot_ty(ctx, elem, sld);
                    let l = ctx.temp();
                    ctx.emit(Instr::Load {
                        dst: l,
                        base: sb,
                        idx: Operand::Const(Const::Int(sld as i128)),
                        ty: sty,
                    });
                    let off = if sld == 0 {
                        Operand::Temp(base_idx)
                    } else {
                        let o = ctx.temp();
                        ctx.emit(Instr::Bin {
                            dst: o,
                            op: BinOp::Add,
                            l: Operand::Temp(base_idx),
                            r: Operand::Const(Const::Int(sld as i128)),
                            ty: cty,
                        });
                        Operand::Temp(o)
                    };
                    let r = ctx.temp();
                    ctx.emit(Instr::Load {
                        dst: r,
                        base: arr,
                        idx: off,
                        ty: sty,
                    });
                    let eq = structural_eq(ctx, Operand::Temp(l), Operand::Temp(r), sty);
                    cond = and_operands(ctx, cond, eq);
                }
                cond
            };
            ctx.terminate(Term::Br {
                cond: eq,
                then_: found_b,
                else_: step_b,
            });
            ctx.switch_to(found_b);
            ctx.emit(Instr::Copy {
                dst: result,
                src: Operand::Const(Const::Bool(true)),
                ty: ctx.sem.tys.bool_,
            });
            ctx.terminate(Term::Jmp(done_b));
            ctx.switch_to(step_b);
            let one = ctx.temp();
            ctx.emit(Instr::Bin {
                dst: one,
                op: BinOp::Add,
                l: Operand::Temp(c),
                r: Operand::Const(Const::Int(1)),
                ty: cty,
            });
            ctx.emit(Instr::Store {
                base: counter,
                idx: Operand::Const(Const::Int(0)),
                src: Operand::Temp(one),
                ty: cty,
            });
            ctx.terminate(Term::Jmp(cond_b));
            ctx.switch_to(done_b);
            Operand::Temp(result)
        }
        _ => {
            ctx.diag(e.span, "in 的右操作数非数组/区间（sem 契约破缺）");
            Operand::Const(Const::Bool(false))
        }
    }
}

/// when：主语单求值 → 源序逐分支 Br 链（模式条件 + guard 内嵌）→
/// 值语境走结果槽（两分支 Store、汇合 Load）；链尾 fallthrough 发
/// unreachable（sem 穷尽性已保证，防御松弛项穿透）
fn lower_when(
    ctx: &mut Ctx,
    e: &ExprWrap,
    subject: Option<&ExprWrap>,
    branches: &[ast::syntax::expr::WhenBranch],
) -> Operand {
    let value_ctx = !ctx.is_unit(ctx.expr_ty(e));
    let result_ty = ctx.expr_ty(e);
    let (subj, sty) = match subject {
        Some(s) => {
            let op = lower_expr(ctx, s);
            let sty = ctx.expr_ty(s);
            // 主语一律物化为 temp：字面量/常量主语经 Copy（复制传播会吃掉），
            // 模式分支的槽 Load 与表达式分支的比较共用同一基址
            let b = match op {
                Operand::Temp(t) => t,
                Operand::Const(c) => {
                    let t = ctx.temp();
                    ctx.emit(Instr::Copy {
                        dst: t,
                        src: Operand::Const(c),
                        ty: sty,
                    });
                    t
                }
            };
            (b, sty)
        }
        None => {
            // 无主语 when（裸 bool 分支等）：subject 恒 true
            let t = ctx.temp();
            ctx.emit(Instr::Copy {
                dst: t,
                src: Operand::Const(Const::Bool(true)),
                ty: ctx.sem.tys.bool_,
            });
            (t, ctx.sem.tys.bool_)
        }
    };
    let result_slot = if value_ctx { Some(ctx.temp()) } else { None };
    let merge_b = ctx.reserve_block();
    let span = e.span;
    let store_result = |ctx: &mut Ctx, slot: Option<TempId>, tail: Option<(Operand, TyId)>| {
        if let (Some(slot), Some((op, from))) = (slot, tail) {
            if crate::layout::is_aggregate(ctx, result_ty) {
                copy_slots(ctx, (slot, 0), (op, 0), from, result_ty, span);
            } else {
                let v = coerce(ctx, op, from, result_ty);
                ctx.emit(Instr::Store {
                    base: slot,
                    idx: Operand::Const(Const::Int(0)),
                    src: v,
                    ty: result_ty,
                });
            }
        }
    };
    for branch in branches {
        let body_b = ctx.reserve_block();
        let next_b = ctx.reserve_block();
        // 分支作用域先推入：模式绑定限本分支（guard + 体）可见，
        // 不泄漏进后续分支链或外层
        ctx.env_push();
        // 条件：else 无条件；表达式分支 = 主语相等；模式分支 = 模式匹配
        match &branch.pattern {
            ast::syntax::expr::WhenPattern::Else => {
                ctx.terminate(Term::Jmp(body_b));
            }
            ast::syntax::expr::WhenPattern::Expression(expr) => {
                let v = lower_expr(ctx, expr);
                let vty = ctx.expr_ty(expr);
                let sv = coerce(ctx, Operand::Temp(subj), sty, vty);
                let vv = coerce(ctx, v, vty, vty);
                let eq = ctx.temp();
                ctx.emit(Instr::Bin {
                    dst: eq,
                    op: BinOp::Eq,
                    l: sv,
                    r: vv,
                    ty: vty,
                });
                ctx.terminate(Term::Br {
                    cond: Operand::Temp(eq),
                    then_: body_b,
                    else_: next_b,
                });
            }
            ast::syntax::expr::WhenPattern::Pattern(pat) => {
                let c = lower_pattern(ctx, pat, Operand::Temp(subj), sty, pat.span);
                ctx.terminate(Term::Br {
                    cond: c,
                    then_: body_b,
                    else_: next_b,
                });
            }
        }
        ctx.switch_to(body_b);
        // guard：附加条件，假则落入下一分支
        if let Some(guard) = &branch.guard {
            let g = lower_expr(ctx, guard);
            let guard_ok = ctx.reserve_block();
            ctx.terminate(Term::Br {
                cond: g,
                then_: guard_ok,
                else_: next_b,
            });
            ctx.switch_to(guard_ok);
        }
        let tail = match &branch.body {
            ast::syntax::expr::WhenBody::Expr(expr) => {
                Some((lower_expr(ctx, expr), ctx.expr_ty(expr)))
            }
            ast::syntax::expr::WhenBody::Block(b) => ctx.lower_block(b),
        };
        store_result(ctx, result_slot, tail);
        ctx.env_pop();
        ctx.terminate(Term::Jmp(merge_b));
        ctx.switch_to(next_b);
    }
    // 穷尽性 sem 已验证；else 存在时链尾不可达但需终止器
    ctx.terminate(Term::Unreachable);
    ctx.switch_to(merge_b);
    match result_slot {
        Some(slot) => {
            if crate::layout::is_aggregate(ctx, result_ty) {
                Operand::Temp(slot)
            } else {
                let dst = ctx.temp();
                ctx.emit(Instr::Load {
                    dst,
                    base: slot,
                    idx: Operand::Const(Const::Int(0)),
                    ty: result_ty,
                });
                // M4 slice 4b (Plan C)：class 汇合结果统一 +1
                if crate::layout::is_class(ctx, result_ty) {
                    ctx.note_stmt_temp(dst, result_ty);
                }
                Operand::Temp(dst)
            }
        }
        None => Operand::Const(Const::Unit),
    }
}

/// ?. 安全调用（标量成员）：flag 检查，null 传播整个表达式
/// （结果型按 sem 定型：Nullable → 新 nullable 值；非 nullable → 诊断）
fn lower_safe_member(ctx: &mut Ctx, e: &ExprWrap, base: &ExprWrap) -> Operand {
    let bty = ctx.expr_ty(base);
    let to = ctx.expr_ty(e);
    let TyKind::Nullable(inner) = ctx.sem.tys.kind(to) else {
        ctx.diag(e.span, "?. 结果非 nullable（sem 契约破缺）");
        return Operand::Const(Const::Unit);
    };
    let inner = *inner;
    let Some(crate::layout::MemberRes::Field { parent, index }) =
        crate::layout::resolve_member(ctx, bty, member_name(e))
    else {
        ctx.diag(e.span, "?. 目标非字段成员（sem v1 方法调用已跳过）");
        return Operand::Const(Const::Unit);
    };
    let bop = lower_expr(ctx, base);
    let b = as_base(ctx, bop, e.span);
    let flag = ctx.temp();
    ctx.emit(Instr::Load {
        dst: flag,
        base: b,
        idx: Operand::Const(Const::Int(0)),
        ty: ctx.i8_ty,
    });
    let ok_b = ctx.reserve_block();
    let null_b = ctx.reserve_block();
    let merge_b = ctx.reserve_block();
    ctx.terminate(Term::Br {
        cond: Operand::Temp(flag),
        then_: ok_b,
        else_: null_b,
    });
    // 非空：字段在 payload 区（slot 1 + 字段偏移）
    ctx.switch_to(ok_b);
    let fty = ctx.sem.defs.get(parent).sig.fields[index].1;
    // M4 slice 4a: base payload is class (bty ?P) -> field via object ptr
    let base_class = matches!(ctx.sem.tys.kind(bty), TyKind::Nullable(bi) if is_class_ty(ctx, *bi));
    let v = if base_class {
        // M4 slice 4a: payload slot 1 = object ptr; field via header offset
        let obj = ctx.temp();
        ctx.emit(Instr::Load {
            dst: obj,
            base: b,
            idx: Operand::Const(Const::Int(1)),
            ty: inner,
        });
        let addr = class_field_addr(ctx, obj, parent, index);
        let fv = ctx.temp();
        ctx.emit(Instr::LoadPtr {
            dst: fv,
            ptr: addr,
            ty: fty,
        });
        Operand::Temp(fv)
    } else {
        let slot = 1 + crate::layout::field_slot(ctx, parent, index);
        read_slots(ctx, b, slot, fty, e.span)
    };
    let out = ctx.temp();
    ctx.emit(Instr::Store {
        base: out,
        idx: Operand::Const(Const::Int(0)),
        src: Operand::Const(Const::Int(1)),
        ty: ctx.i8_ty,
    });
    copy_slots(ctx, (out, 1), (v, 0), fty, inner, e.span);
    ctx.terminate(Term::Jmp(merge_b));
    // 空：flag=0，payload 不初始化
    ctx.switch_to(null_b);
    ctx.emit(Instr::Store {
        base: out,
        idx: Operand::Const(Const::Int(0)),
        src: Operand::Const(Const::Int(0)),
        ty: ctx.i8_ty,
    });
    ctx.terminate(Term::Jmp(merge_b));
    ctx.switch_to(merge_b);
    Operand::Temp(out)
}

/// ?? elvis：lhs 为 nullable → flag 分支（some 取 payload，none 取 rhs）；
/// 非 nullable lhs 恒取 lhs
fn lower_elvis(ctx: &mut Ctx, e: &ExprWrap, lhs: &ExprWrap, rhs: &ExprWrap) -> Operand {
    let lty = ctx.expr_ty(lhs);
    let to = ctx.expr_ty(e);
    let lop = lower_expr(ctx, lhs);
    let TyKind::Nullable(s_inner) = ctx.sem.tys.kind(lty) else {
        return coerce(ctx, lop, lty, to);
    };
    let s_inner = *s_inner;
    let lb = as_base(ctx, lop, e.span);
    let flag = ctx.temp();
    ctx.emit(Instr::Load {
        dst: flag,
        base: lb,
        idx: Operand::Const(Const::Int(0)),
        ty: ctx.i8_ty,
    });
    let some_b = ctx.reserve_block();
    let none_b = ctx.reserve_block();
    let merge_b = ctx.reserve_block();
    ctx.terminate(Term::Br {
        cond: Operand::Temp(flag),
        then_: some_b,
        else_: none_b,
    });
    // 结果槽：Store 发在各分支内、汇合只读（phi 绕法，同 lower_if——
    // 双 Store 都发在汇合块会让后写恒覆盖先写，?? 恒取右值）
    let slot = ctx.temp();
    let n = crate::layout::scalar_slots(ctx, to).max(1);
    ctx.switch_to(some_b);
    let some_v = if !crate::layout::is_aggregate(ctx, s_inner) {
        let sty = crate::layout::slot_ty(ctx, lty, 1);
        let t = ctx.temp();
        ctx.emit(Instr::Load {
            dst: t,
            base: lb,
            idx: Operand::Const(Const::Int(1)),
            ty: sty,
        });
        coerce(ctx, Operand::Temp(t), sty, to)
    } else {
        let base = ctx.temp();
        copy_slots(ctx, (base, 0), (Operand::Temp(lb), 1), s_inner, to, e.span);
        Operand::Temp(base)
    };
    // M4 slice 4b (Plan C)：class 的 some 分支借用 lhs payload——
    // 显式 retain，使 ?? 结果统一 +1（none 分支转移/retain 同规则）
    if crate::layout::is_class(ctx, to)
        && let Operand::Temp(t) = some_v
    {
        ctx.emit_class_retain(crate::lower::Place::Temp(t), to);
    }
    if !crate::layout::is_aggregate(ctx, to) {
        ctx.emit(Instr::Store {
            base: slot,
            idx: Operand::Const(Const::Int(0)),
            src: some_v,
            ty: to,
        });
    } else {
        let sb = as_base(ctx, some_v, e.span);
        copy_slots(ctx, (slot, 0), (Operand::Temp(sb), 0), to, to, e.span);
    }
    ctx.terminate(Term::Jmp(merge_b));
    ctx.switch_to(none_b);
    let none_v = {
        let v = lower_expr(ctx, rhs);
        coerce(ctx, v, ctx.expr_ty(rhs), to)
    };
    if crate::layout::is_class(ctx, to) {
        if plus1_origin(ctx, rhs) {
            let _ = ctx.take_stmt_temp(&none_v);
        } else if let Operand::Temp(t) = none_v {
            ctx.emit_class_retain(crate::lower::Place::Temp(t), to);
        }
    }
    if n <= 1 {
        ctx.emit(Instr::Store {
            base: slot,
            idx: Operand::Const(Const::Int(0)),
            src: none_v,
            ty: to,
        });
    } else {
        let nb = as_base(ctx, none_v, e.span);
        copy_slots(ctx, (slot, 0), (Operand::Temp(nb), 0), to, to, e.span);
    }
    ctx.terminate(Term::Jmp(merge_b));
    ctx.switch_to(merge_b);
    if n <= 1 {
        let dst = ctx.temp();
        ctx.emit(Instr::Load {
            dst,
            base: slot,
            idx: Operand::Const(Const::Int(0)),
            ty: to,
        });
        // M4 slice 4b (Plan C)：class ?? 结果统一 +1
        if crate::layout::is_class(ctx, to) {
            ctx.note_stmt_temp(dst, to);
        }
        Operand::Temp(dst)
    } else {
        Operand::Temp(slot)
    }
}

/// 区间值物化 lo..hi：两槽基址（slot0 = lo，slot1 = hi）。
/// `in` 的物化路径与 for-over-Range 值消费此形态；类型不区分开闭
/// （Range(T) 无 exclusivity 信息，消费端按闭区间，见 opt-notes corner）
fn range_value(ctx: &mut Ctx, e: &ExprWrap, lhs: &ExprWrap, rhs: &ExprWrap) -> Operand {
    let rty = ctx.expr_ty(e);
    let inner = match ctx.sem.tys.kind(rty) {
        TyKind::Range(inner) => *inner,
        _ => {
            ctx.diag(e.span, "区间表达式类型非 Range（sem 契约破缺）");
            return Operand::Const(Const::Unit);
        }
    };
    let lo_op = lower_expr(ctx, lhs);
    let lo = coerce(ctx, lo_op, ctx.expr_ty(lhs), inner);
    let hi_op = lower_expr(ctx, rhs);
    let hi = coerce(ctx, hi_op, ctx.expr_ty(rhs), inner);
    let base = ctx.temp();
    ctx.emit(Instr::Store {
        base,
        idx: Operand::Const(Const::Int(0)),
        src: lo,
        ty: inner,
    });
    let n = crate::layout::scalar_slots(ctx, inner).max(1);
    ctx.emit(Instr::Store {
        base,
        idx: Operand::Const(Const::Int(n as i128)),
        src: hi,
        ty: inner,
    });
    // 槽 2n = 开闭标记（1 = 含上界）：物化后 `in`/`for` 仍区分 `..`/`..=`
    let inclusive = matches!(
        &e.kind,
        ExprKind::Binary {
            op: OperatorKind::DotDotEq,
            ..
        }
    );
    ctx.emit(Instr::Store {
        base,
        idx: Operand::Const(Const::Int((2 * n) as i128)),
        src: Operand::Const(Const::Int(if inclusive { 1 } else { 0 })),
        ty: ctx.i8_ty,
    });
    Operand::Temp(base)
}

/// if/while 条件：Expr 直接 lowering；Val（val 模式 = 表达式）开新作用域
/// 绑定模式名（sem：val 条件绑定限块内可见），is 条件的绑定同样限 then 分支
pub(crate) fn lower_cond(ctx: &mut Ctx, cond: &ast::syntax::expr::Condition) -> (Operand, bool) {
    use ast::syntax::expr::Condition;
    match cond {
        Condition::Expr(e) => {
            let scoped = matches!(e.kind, ExprKind::Is { .. });
            if scoped {
                // is 绑定限 then 分支：先推作用域再 lowering，
                // 调用方在 then 分支后配对弹出（lower.rs）
                ctx.env_push();
            }
            (lower_expr(ctx, e), scoped)
        }
        Condition::Val { pattern, expr } => {
            let sty = ctx.expr_ty(expr);
            let s = lower_expr(ctx, expr);
            ctx.env_push();
            let c = lower_pattern(ctx, pattern, s, sty, pattern.span);
            (c, true)
        }
    }
}

// ---- M3：字符串插值 + 赋值补全 ----

/// 字符串插值 "a${x}b"：逐 part 经运行时 starry_str_conv（源类型在
/// Param 的 ty 上），再左结合 starry_str_concat 折叠。runtime 以
/// extern 符号存在（llvme declare），std-only 红线不进 crate
fn string_interp(ctx: &mut Ctx, e: &ExprWrap, parts: &[ExprWrap]) -> Operand {
    let str_ty = ctx.expr_ty(e);
    let mut acc: Option<TempId> = None;
    for part in parts {
        let pty = ctx.expr_ty(part);
        if crate::layout::is_aggregate(ctx, pty) || crate::layout::scalar_slots(ctx, pty) != 1 {
            ctx.diag(part.span, "聚合 part 的字符串化在 M-later");
            continue;
        }
        if matches!(ctx.sem.tys.kind(pty), TyKind::Nullable(_)) {
            ctx.diag(part.span, "nullable part 的字符串化在 M-later");
            continue;
        }
        let v = lower_expr(ctx, part);
        ctx.emit(Instr::Param { a: v, ty: pty });
        let conv = ctx.temp();
        let runtime = if matches!(ctx.sem.tys.kind(pty), TyKind::Char) {
            RuntimeFn::StrConvChar
        } else {
            RuntimeFn::StrConv
        };
        ctx.emit(Instr::Call {
            dst: Some(conv),
            callee: Callee::Runtime(runtime),
            ty: str_ty,
        });
        acc = Some(match acc {
            None => conv,
            Some(prev) => {
                ctx.emit(Instr::Param {
                    a: Operand::Temp(prev),
                    ty: str_ty,
                });
                ctx.emit(Instr::Param {
                    a: Operand::Temp(conv),
                    ty: str_ty,
                });
                let d = ctx.temp();
                ctx.emit(Instr::Call {
                    dst: Some(d),
                    callee: Callee::Runtime(RuntimeFn::StrConcat),
                    ty: str_ty,
                });
                d
            }
        });
    }
    match acc {
        Some(t) => Operand::Temp(t),
        None => {
            ctx.diag(e.span, "插值无可用 part（诊断后置信下游不启动）");
            Operand::Const(Const::Unit)
        }
    }
}

/// 复合赋值算子 → 底层二元算子
pub(crate) fn compound_binop(op: OperatorKind) -> Option<BinOp> {
    Some(match op {
        OperatorKind::PlusEq => BinOp::Add,
        OperatorKind::MinusEq => BinOp::Sub,
        OperatorKind::StarEq => BinOp::Mul,
        OperatorKind::SlashEq => BinOp::Div,
        OperatorKind::PercentEq => BinOp::Rem,
        OperatorKind::BitAndEq => BinOp::BitAnd,
        OperatorKind::BitOrEq => BinOp::BitOr,
        OperatorKind::BitXorEq => BinOp::BitXor,
        OperatorKind::ShlEq => BinOp::Shl,
        OperatorKind::ShrEq => BinOp::Shr,
        _ => return None,
    })
}

/// 赋值基座解算（写穿透）：Ident 直通 Slot/Temp 原存储——var 的
/// read_place 是快照拷贝，经它写入会落在拷贝上（赋值丢失）。
/// nullable 基座与非 Ident 基座（成员/索引链）的写穿透在 M-later
/// （?. 赋值同样诊断，语义一致）
fn lvalue_base(ctx: &mut Ctx, base: &ExprWrap, span: Span) -> Option<TempId> {
    if matches!(ctx.sem.tys.kind(ctx.expr_ty(base)), TyKind::Nullable(_)) {
        ctx.diag(span, "nullable 基座的成员/索引赋值在 M-later");
        return None;
    }
    match &base.kind {
        ExprKind::Ident(name) => match ctx.lookup(*name) {
            Some((Place::Slot(s), _)) => Some(s),
            Some((Place::Temp(t), _)) => Some(t),
            Some((Place::Global(_), _)) => {
                ctx.diag(span, "全局聚合基座的成员/索引赋值在 M-later");
                None
            }
            None => {
                ctx.diag(span, "赋值基座不在环境（sem 契约破缺）");
                None
            }
        },
        _ => {
            ctx.diag(span, "复合基座（成员/索引链）的赋值在 M-later");
            None
        }
    }
}

/// 赋值目标写入：Ident（标量 Store / 聚合逐槽拷贝）、成员字段
/// （字段槽写入）、数组索引（slot 寻址）。safe 成员、聚合元素/
/// 字段赋值在 M-later。v 已由调用方 coerce 到目标类型
pub(crate) fn assign_to(ctx: &mut Ctx, target: &ExprWrap, v: Operand, span: Span) {
    match &target.kind {
        ExprKind::Ident(name) => {
            let Some((place, ty)) = ctx.lookup(*name) else {
                ctx.diag(span, "赋值目标不在环境（sem 契约破缺）");
                return;
            };
            match place {
                Place::Slot(slot) => {
                    if crate::layout::is_aggregate(ctx, ty) {
                        copy_slots(ctx, (slot, 0), (v, 0), ty, ty, span);
                    } else {
                        ctx.emit(Instr::Store {
                            base: slot,
                            idx: Operand::Const(Const::Int(0)),
                            src: v,
                            ty,
                        });
                    }
                }
                Place::Global(global) => {
                    if crate::layout::is_aggregate(ctx, ty) || ctx.is_unit(ty) {
                        ctx.diag(span, "聚合/unit 全局赋值的 lowering 在 M-later");
                        return;
                    }
                    ctx.emit(Instr::StoreGlobal { global, src: v, ty });
                }
                Place::Temp(_) => {
                    ctx.diag(span, "对不可变绑定赋值穿透 sem 门禁（SemAssignToVal 漏报）");
                }
            }
        }
        ExprKind::Deref { expr } => {
            let op = lower_expr(ctx, expr);
            let ptr = match op {
                Operand::Temp(t) => t,
                Operand::Const(_) => {
                    ctx.diag(span, "解引赋值目标为常量（sem 契约破缺）");
                    return;
                }
            };
            let ty = ctx
                .sem
                .expr_ty
                .get(target.id)
                .copied()
                .unwrap_or(ctx.sem.tys.unit);
            // S2/N2 审计（nullable-abi-audit.md §3.2）：写入指向可空的
            // 指针——StorePtr 按 Cell(i64) 只写 flag 槽（payload 缺失）
            if matches!(ctx.sem.tys.kind(ty), TyKind::Nullable(..)) {
                ctx.diag(span, "write through a pointer to a nullable type is not supported in v1 (single-slot store drops the payload)");
                return;
            }
            ctx.emit(Instr::StorePtr { ptr, src: v, ty });
        }
        ExprKind::Member {
            base, safe: false, ..
        } => {
            let Some(crate::layout::MemberRes::Field { parent, index }) =
                crate::layout::resolve_member(ctx, ctx.expr_ty(base), member_name(target))
            else {
                ctx.diag(span, "?. 目标或字段解析失败（sem 契约破缺）");
                return;
            };
            let fty = ctx.sem.defs.get(parent).sig.fields[index].1;
            if is_class_ty(ctx, ctx.expr_ty(base)) {
                // M4 slice 1：class 字段经对象指针 + 偏移写
                let Some(obj) = lvalue_base(ctx, base, span) else {
                    return;
                };
                let addr = class_field_addr(ctx, obj, parent, index);
                ctx.emit(Instr::StorePtr {
                    ptr: addr,
                    src: v,
                    ty: fty,
                });
                return;
            }
            let Some(b) = lvalue_base(ctx, base, span) else {
                return;
            };
            let off = crate::layout::field_slot(ctx, parent, index);
            if crate::layout::is_aggregate(ctx, fty) {
                copy_slots(ctx, (b, off), (v, 0), fty, fty, span);
            } else {
                ctx.emit(Instr::Store {
                    base: b,
                    idx: Operand::Const(Const::Int(off as i128)),
                    src: v,
                    ty: fty,
                });
            }
        }
        ExprKind::Member { safe: true, .. } => {
            ctx.diag(span, "?. 赋值目标的 lowering 未支持（sem 契约破缺）");
        }
        ExprKind::Index {
            base,
            index,
            const_index,
        } => {
            let bty = ctx.expr_ty(base);
            let Some(b) = lvalue_base(ctx, base, span) else {
                return;
            };
            if let Some((elem, off)) = tuple_index_slot(ctx, bty, *const_index, span) {
                if crate::layout::is_aggregate(ctx, elem) {
                    copy_slots(ctx, (b, off), (v, 0), elem, elem, span);
                } else {
                    ctx.emit(Instr::Store {
                        base: b,
                        idx: Operand::Const(Const::Int(off as i128)),
                        src: v,
                        ty: elem,
                    });
                }
                return;
            }
            let TyKind::Array(elem, _) = ctx.sem.tys.kind(bty) else {
                ctx.diag(span, "索引赋值目标非数组/元组（sem 契约破缺）");
                return;
            };
            let elem = *elem;
            let es = crate::layout::scalar_slots(ctx, elem);
            let iop = lower_expr(ctx, index);
            if !crate::layout::is_aggregate(ctx, elem) {
                // S3：多槽标量元素（i128）索引 × es stride
                let iop = if es > 1 {
                    let ity = ctx.expr_ty(index);
                    let m = ctx.temp();
                    ctx.emit(Instr::Bin {
                        dst: m,
                        op: BinOp::Mul,
                        l: iop,
                        r: Operand::Const(Const::Int(es as i128)),
                        ty: ity,
                    });
                    Operand::Temp(m)
                } else {
                    iop
                };
                ctx.emit(Instr::Store {
                    base: b,
                    idx: iop,
                    src: v,
                    ty: elem,
                });
            } else {
                let sb = as_base(ctx, v, span);
                let ity = ctx.expr_ty(index);
                let base_idx = ctx.temp();
                ctx.emit(Instr::Bin {
                    dst: base_idx,
                    op: BinOp::Mul,
                    l: iop,
                    r: Operand::Const(Const::Int(es as i128)),
                    ty: ity,
                });
                for sld in 0..es {
                    let sty = crate::layout::slot_ty(ctx, elem, sld);
                    let t = ctx.temp();
                    ctx.emit(Instr::Load {
                        dst: t,
                        base: sb,
                        idx: Operand::Const(Const::Int(sld as i128)),
                        ty: sty,
                    });
                    let off = if sld == 0 {
                        Operand::Temp(base_idx)
                    } else {
                        let o = ctx.temp();
                        ctx.emit(Instr::Bin {
                            dst: o,
                            op: BinOp::Add,
                            l: Operand::Temp(base_idx),
                            r: Operand::Const(Const::Int(sld as i128)),
                            ty: ity,
                        });
                        Operand::Temp(o)
                    };
                    ctx.emit(Instr::Store {
                        base: b,
                        idx: off,
                        src: Operand::Temp(t),
                        ty: sty,
                    });
                }
            }
        }
        _ => ctx.diag(span, "该赋值目标无 lowering 路径（sem 契约破缺）"),
    }
}
