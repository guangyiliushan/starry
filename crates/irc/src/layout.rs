//! 槽位布局（opt-notes「数据布局」的实现）：值 = 基址 temp + Load/Store 槽寻址。
//!
//! 规则：标量 1 槽（8 字节，llvme 侧按需 bitcast）；struct 按字段序摊平；
//! 和类型（enum/nullable）槽 0 = i8 tag/flag，payload 自槽 1；
//! enum payload 布局 = 最大槽数变体（并列取先者，确定性）；
//! array = len × 元素槽；tuple 按元素序；range = lo,hi 两槽。

use sem::{DefId, DefKind, TyId, TyKind};

use crate::lower::Ctx;

/// ty 的摊平标量槽数（和类型含 tag 槽；unit/null = 0）
pub(crate) fn scalar_slots(ctx: &Ctx, ty: TyId) -> usize {
    match ctx.sem.tys.kind(ty) {
        TyKind::Int(_) | TyKind::Float(_) | TyKind::Bool | TyKind::Char | TyKind::String => 1,
        TyKind::Unit | TyKind::Null => 0,
        TyKind::Array(elem, len) => scalar_slots(ctx, *elem) * *len as usize,
        TyKind::Tuple(elems) => elems.iter().map(|e| scalar_slots(ctx, *e)).sum(),
        TyKind::Range(inner) => scalar_slots(ctx, *inner) * 2,
        TyKind::Nullable(inner) => 1 + scalar_slots(ctx, *inner),
        TyKind::Named(def, _) => match ctx.sem.defs.get(*def).kind {
            DefKind::Struct | DefKind::Class => ctx
                .sem
                .defs
                .get(*def)
                .sig
                .fields
                .iter()
                .map(|(_, t)| scalar_slots(ctx, *t))
                .sum(),
            DefKind::Enum => {
                let def = ctx.sem.defs.get(*def);
                1 + def
                    .sig
                    .variants
                    .iter()
                    .map(|(_, params)| {
                        params
                            .iter()
                            .map(|(_, t)| scalar_slots(ctx, *t))
                            .sum::<usize>()
                    })
                    .max()
                    .unwrap_or(0)
            }
            // 其余 Named（builtin Own/Shared、alias 目标）按单槽防御
            _ => 1,
        },
        // Fn/TypeObject/Unknown/Error：不应出现在值布局中
        _ => 1,
    }
}

/// 槽 offset 处的标量类型（Load/Store 的 ty；llvme 的槽类型）
pub(crate) fn slot_ty(ctx: &Ctx, ty: TyId, slot: usize) -> TyId {
    match ctx.sem.tys.kind(ty) {
        TyKind::Array(elem, _) => {
            let es = scalar_slots(ctx, *elem).max(1);
            slot_ty(ctx, *elem, slot % es)
        }
        TyKind::Tuple(elems) => {
            let mut rest = slot;
            for e in elems {
                let n = scalar_slots(ctx, *e);
                if rest < n {
                    return slot_ty(ctx, *e, rest);
                }
                rest -= n;
            }
            ctx.sem.tys.unit
        }
        TyKind::Range(inner) => {
            let n = scalar_slots(ctx, *inner).max(1);
            slot_ty(ctx, *inner, slot % n)
        }
        TyKind::Nullable(inner) => {
            if slot == 0 {
                ctx.i8_ty
            } else {
                slot_ty(ctx, *inner, slot - 1)
            }
        }
        TyKind::Named(def, _) => match ctx.sem.defs.get(*def).kind {
            DefKind::Struct | DefKind::Class => {
                let mut rest = slot;
                for (_, t) in &ctx.sem.defs.get(*def).sig.fields {
                    let n = scalar_slots(ctx, *t);
                    if rest < n {
                        return slot_ty(ctx, *t, rest);
                    }
                    rest -= n;
                }
                ctx.sem.tys.unit
            }
            DefKind::Enum => {
                if slot == 0 {
                    return ctx.i8_ty;
                }
                let def = ctx.sem.defs.get(*def);
                let v = max_variant(ctx, def);
                let mut rest = slot - 1;
                for (_, t) in &def.sig.variants[v].1 {
                    let n = scalar_slots(ctx, *t);
                    if rest < n {
                        return slot_ty(ctx, *t, rest);
                    }
                    rest -= n;
                }
                ctx.sem.tys.unit
            }
            _ => ty,
        },
        // 标量：槽 0 即自身
        _ => ty,
    }
}

/// struct 字段的槽偏移（前序字段槽数和）
pub(crate) fn field_slot(ctx: &Ctx, def: DefId, index: usize) -> usize {
    ctx.sem.defs.get(def).sig.fields[..index]
        .iter()
        .map(|(_, t)| scalar_slots(ctx, *t))
        .sum()
}

/// enum 变体的第 i 个形参的槽偏移（变体 payload 内，不含 tag 槽）
pub(crate) fn variant_param_slot(ctx: &Ctx, def: DefId, variant: usize, index: usize) -> usize {
    ctx.sem.defs.get(def).sig.variants[variant].1[..index]
        .iter()
        .map(|(_, t)| scalar_slots(ctx, *t))
        .sum()
}

/// 最大 payload 槽数的变体序号（并列取先，保证输出可复现）
fn max_variant(ctx: &Ctx, def: &sem::Def) -> usize {
    let mut best = 0;
    let mut best_n = 0;
    for (i, (_, params)) in def.sig.variants.iter().enumerate() {
        let n: usize = params.iter().map(|(_, t)| scalar_slots(ctx, *t)).sum();
        if n > best_n {
            best = i;
            best_n = n;
        }
    }
    best
}

/// 成员解析结果：struct 字段或 enum 变体（按名解析，确定性）
#[derive(Debug, Clone, Copy)]
pub(crate) enum MemberRes {
    Field { parent: DefId, index: usize },
    Variant { parent: DefId, index: usize },
}

/// 按名解析类型成员。sem v1 不公开字段/变体的 expr_def 侧表——命名类型
/// 的成员名在 def 签名内唯一，本地解析与 sem 的 member_lookup 等价。
/// 类型别名经 `Def.alias_resolved` 预解析目标（sem 四轮回修产物）。
pub(crate) fn resolve_member(
    ctx: &Ctx,
    base_ty: TyId,
    name: ast::interner::Symbol,
) -> Option<MemberRes> {
    let mut def = match ctx.sem.tys.kind(base_ty) {
        TyKind::Named(d, _) | TyKind::TypeObject(d) => *d,
        _ => return None,
    };
    if let DefKind::TypeAlias = ctx.sem.defs.get(def).kind {
        def = ctx.sem.defs.get(def).alias_resolved?;
    }
    match &ctx.sem.defs.get(def).kind {
        DefKind::Struct | DefKind::Class => ctx
            .sem
            .defs
            .get(def)
            .sig
            .fields
            .iter()
            .position(|(n, _)| *n == name)
            .map(|index| MemberRes::Field { parent: def, index }),
        DefKind::Enum => ctx
            .sem
            .defs
            .get(def)
            .sig
            .variants
            .iter()
            .position(|(n, _)| *n == name)
            .map(|index| MemberRes::Variant { parent: def, index }),
        _ => None,
    }
}
