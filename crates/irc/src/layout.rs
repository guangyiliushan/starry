//! 槽位布局（opt-notes「数据布局」的实现）：值 = 基址 temp + Load/Store 槽寻址。
//!
//! 规则：标量 1 槽（8 字节，llvme 侧按需 bitcast）；struct 按字段序摊平；
//! 和类型（enum/nullable）槽 0 = i8 tag/flag，payload 自槽 1；
//! enum payload 布局 = 最大槽数变体（并列取先者，确定性）；
//! array = len × 元素槽；tuple 按元素序；range = lo,hi 两槽。

use sem::{DefArena, DefId, DefKind, TyArena, TyId, TyKind};

/// 槽布局查询的视图：Ctx（irc 内部）与 llvme 的消费端各自实现。
/// 公开为 llvme 的跨 crate 契约（opt-notes：槽布局由 irc/llvme 共享）
pub trait LayoutView {
    fn tys(&self) -> &TyArena;
    fn defs(&self) -> &DefArena;
    fn i8_ty(&self) -> TyId;
}

/// 值的表示形态：聚合走「基址 temp + Load/Store 槽寻址」，标量按值传递。
///
/// 判据是**类型种类**而不是槽数：`struct P { x: int }` 与 `int` 同为 1 槽，
/// 但前者是基址、后者是值。用 `scalar_slots(ty) <= 1` 当「标量」会把单槽
/// 聚合折成标量值，成员读取随即把该值当基址用（非法 IR + 丢字段）。
/// 与 llvme 的 `TyClass` 同口径。
/// M4 slice 1：ty 是否为 class 值（对象指针标量化）
pub fn is_class(v: &impl LayoutView, ty: TyId) -> bool {
    matches!(
        v.tys().kind(ty),
        TyKind::Named(def, _) if matches!(v.defs().get(*def).kind, DefKind::Class)
    )
}

pub fn is_aggregate(v: &impl LayoutView, ty: TyId) -> bool {
    // M4 slice 1: class values are scalarized object pointers
    if let TyKind::Named(def, _) = v.tys().kind(ty)
        && matches!(v.defs().get(*def).kind, DefKind::Class)
    {
        return false;
    }
    !matches!(
        v.tys().kind(ty),
        TyKind::Int(_)
            | TyKind::Float(_)
            | TyKind::Bool
            | TyKind::Char
            | TyKind::String
            | TyKind::Unit
            | TyKind::Null
    )
}

/// ty 的摊平标量槽数（和类型含 tag 槽；unit/null = 0）
pub fn scalar_slots(v: &impl LayoutView, ty: TyId) -> usize {
    match v.tys().kind(ty) {
        TyKind::Int(_) | TyKind::Float(_) | TyKind::Bool | TyKind::Char | TyKind::String => 1,
        TyKind::Unit | TyKind::Null => 0,
        TyKind::Array(elem, len) => scalar_slots(v, *elem) * *len as usize,
        TyKind::Tuple(elems) => elems.iter().map(|e| scalar_slots(v, *e)).sum(),
        TyKind::Range(inner) => scalar_slots(v, *inner) * 2 + 1,
        TyKind::Nullable(inner) => 1 + scalar_slots(v, *inner),
        TyKind::Named(def, _) => match v.defs().get(*def).kind {
            // M4 slice 4a：class 值 = 标量化对象指针（与 is_aggregate 同口径），
            // nullable class 的 payload 槽因此是 1 个 ptr 槽
            DefKind::Class => 1,
            DefKind::Struct => v
                .defs()
                .get(*def)
                .sig
                .fields
                .iter()
                .map(|(_, t)| scalar_slots(v, *t))
                .sum(),
            DefKind::Enum => {
                let def = v.defs().get(*def);
                1 + def
                    .sig
                    .variants
                    .iter()
                    .map(|(_, params)| {
                        params
                            .iter()
                            .map(|(_, t)| scalar_slots(v, *t))
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
pub fn slot_ty(v: &impl LayoutView, ty: TyId, slot: usize) -> TyId {
    match v.tys().kind(ty) {
        TyKind::Array(elem, _) => {
            let es = scalar_slots(v, *elem).max(1);
            slot_ty(v, *elem, slot % es)
        }
        TyKind::Tuple(elems) => {
            let mut rest = slot;
            for e in elems {
                let n = scalar_slots(v, *e);
                if rest < n {
                    return slot_ty(v, *e, rest);
                }
                rest -= n;
            }
            v.tys().unit
        }
        TyKind::Range(inner) => {
            let n = scalar_slots(v, *inner).max(1);
            if slot >= 2 * n {
                v.i8_ty()
            } else {
                slot_ty(v, *inner, slot % n)
            }
        }
        TyKind::Nullable(inner) => {
            if slot == 0 {
                v.i8_ty()
            } else {
                slot_ty(v, *inner, slot - 1)
            }
        }
        TyKind::Named(def, _) => match v.defs().get(*def).kind {
            // M4 slice 4a：class 的槽 0 = 值自身（ptr），与标量同纪律
            DefKind::Class => {
                if slot == 0 {
                    ty
                } else {
                    v.tys().unit
                }
            }
            DefKind::Struct => {
                let mut rest = slot;
                for (_, t) in &v.defs().get(*def).sig.fields {
                    let n = scalar_slots(v, *t);
                    if rest < n {
                        return slot_ty(v, *t, rest);
                    }
                    rest -= n;
                }
                v.tys().unit
            }
            DefKind::Enum => {
                if slot == 0 {
                    return v.i8_ty();
                }
                let def = v.defs().get(*def);
                let mv = max_variant(v, def);
                let mut rest = slot - 1;
                for (_, t) in &def.sig.variants[mv].1 {
                    let n = scalar_slots(v, *t);
                    if rest < n {
                        return slot_ty(v, *t, rest);
                    }
                    rest -= n;
                }
                v.tys().unit
            }
            _ => ty,
        },
        // 标量：槽 0 即自身
        _ => ty,
    }
}

/// struct 字段的槽偏移（前序字段槽数和）
pub fn field_slot(v: &impl LayoutView, def: DefId, index: usize) -> usize {
    v.defs().get(def).sig.fields[..index]
        .iter()
        .map(|(_, t)| scalar_slots(v, *t))
        .sum()
}

/// enum 变体的第 i 个形参的槽偏移（变体 payload 内，不含 tag 槽）
pub fn variant_param_slot(v: &impl LayoutView, def: DefId, variant: usize, index: usize) -> usize {
    v.defs().get(def).sig.variants[variant].1[..index]
        .iter()
        .map(|(_, t)| scalar_slots(v, *t))
        .sum()
}

/// 最大 payload 槽数的变体序号（并列取先，保证输出可复现）
fn max_variant(v: &impl LayoutView, def: &sem::Def) -> usize {
    let mut best = 0;
    let mut best_n = 0;
    for (i, (_, params)) in def.sig.variants.iter().enumerate() {
        let n: usize = params.iter().map(|(_, t)| scalar_slots(v, *t)).sum();
        if n > best_n {
            best = i;
            best_n = n;
        }
    }
    best
}

/// 成员解析结果：struct 字段或 enum 变体（按名解析，确定性）
#[derive(Debug, Clone, Copy)]
pub enum MemberRes {
    Field { parent: DefId, index: usize },
    Variant { parent: DefId, index: usize },
}

/// 按名解析类型成员。sem v1 不公开字段/变体的 expr_def 侧表——命名类型
/// 的成员名在 def 签名内唯一，本地解析与 sem 的 member_lookup 等价。
/// 类型别名经 `Def.alias_resolved` 预解析目标（sem 四轮回修产物）。
pub fn resolve_member(
    v: &impl LayoutView,
    base_ty: TyId,
    name: ast::interner::Symbol,
) -> Option<MemberRes> {
    // nullable 剥壳：?. 安全调用的 base 是 T?，成员属于内层 T
    let mut ty = base_ty;
    if let TyKind::Nullable(inner) = v.tys().kind(ty) {
        ty = *inner;
    }
    let mut def = match v.tys().kind(ty) {
        TyKind::Named(d, _) | TyKind::TypeObject(d) => *d,
        _ => return None,
    };
    if let DefKind::TypeAlias = v.defs().get(def).kind {
        def = v.defs().get(def).alias_resolved?;
    }
    match &v.defs().get(def).kind {
        DefKind::Struct | DefKind::Class => v
            .defs()
            .get(def)
            .sig
            .fields
            .iter()
            .position(|(n, _)| *n == name)
            .map(|index| MemberRes::Field { parent: def, index }),
        DefKind::Enum => v
            .defs()
            .get(def)
            .sig
            .variants
            .iter()
            .position(|(n, _)| *n == name)
            .map(|index| MemberRes::Variant { parent: def, index }),
        _ => None,
    }
}
