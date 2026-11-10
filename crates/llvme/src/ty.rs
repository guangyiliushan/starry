//! LLVM 类型映射。
//!
//! 槽模型的关键简化（2026-10-27 llvme 落地时钉死）：**聚合值的
//! LLVM 类型是 `[n x i64]`；只有含 i128 成员的聚合才加宽为
//! `[n x i128]`**（n = scalar_slots）。tag/flag 是槽 0 里的 i8（写时
//! 就地 `store i8`），payload 自槽 1——记录型与和型的槽寻址由此完全
//! 统一（slot i = gep [n x E], p, 0, i），无命名类型定义，结构等价的
//! 类型在 LLVM 侧天然可互换。

use ast::interner::Interner;
use irc::layout::LayoutView;
use sem::{DefArena, DefKind, FloatKind, IntKind, TyArena, TyId, TyKind};

/// llvme 侧的 LayoutView 实现（irc::layout 的跨 crate 消费端）
pub struct Lc<'a> {
    pub tys: &'a TyArena,
    pub defs: &'a sem::DefArena,
    #[allow(dead_code)] // 契约完整性：scalar_slots 不消费 i8，emit 端写死 i8
    pub interner: &'a Interner,
}

impl<'a> LayoutView for Lc<'a> {
    fn tys(&self) -> &TyArena {
        self.tys
    }
    fn defs(&self) -> &sem::DefArena {
        self.defs
    }
    fn i8_ty(&self) -> TyId {
        // scalar_slots 不消费 i8_ty（仅 slot_ty 用）；占位 unit
        self.tys.unit
    }
}

/// 标量的 LLVM 类型名；聚合调用点用 `agg_ty`
pub fn scalar_ty(tys: &TyArena, defs: &DefArena, ty: TyId) -> String {
    match tys.kind(ty) {
        TyKind::Int(IntKind::I4) => "i4".into(),
        TyKind::Int(IntKind::I8) => "i8".into(),
        TyKind::Int(IntKind::I16) => "i16".into(),
        TyKind::Int(IntKind::I32) => "i32".into(),
        TyKind::Int(IntKind::I64) => "i64".into(),
        TyKind::Int(IntKind::I128) => "i128".into(),
        TyKind::Float(FloatKind::F32) => "float".into(),
        TyKind::Float(FloatKind::F64) => "double".into(),
        TyKind::Bool => "i1".into(),
        TyKind::Char => "i32".into(),
        TyKind::String => "i8*".into(),
        // M8 slice 1: function value = opaque function pointer
        TyKind::Fn(..) => "ptr".into(),
        TyKind::Param(_) => "i64".into(),
        // 引用/裸指针值 = 地址；不透明指针风格（LLVM 15+）
        TyKind::Ref { .. } => "ptr".into(),
        TyKind::Pointer { .. } => "ptr".into(),
        // M4 slice 1: class value = object ptr (header lives on the heap)
        TyKind::Named(def, _) if matches!(defs.get(*def).kind, DefKind::Class) => "ptr".into(),
        TyKind::Unit => "void".into(),
        TyKind::Null => "i64".into(),
        // 聚合出现在标量位 = 契约破缺（irc 已门禁），兜底 i64
        _ => "i64".into(),
    }
}

/// 值类型按种类分类：标量经值，聚合经指针
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TyClass {
    Scalar,
    Agg,
}

pub fn class_of(tys: &TyArena, defs: &DefArena, ty: TyId) -> TyClass {
    match tys.kind(ty) {
        TyKind::Int(_)
        | TyKind::Float(_)
        | TyKind::Bool
        | TyKind::Char
        | TyKind::String
        | TyKind::Unit
        | TyKind::Null
        | TyKind::Ref { .. }
        | TyKind::Pointer { .. } => TyClass::Scalar,
        // M4 slice 1: class value = scalarized object ptr (not inline agg)
        TyKind::Named(def, _) if matches!(defs.get(*def).kind, DefKind::Class) => TyClass::Scalar,
        // M8 slice 1: function value = scalar function pointer
        TyKind::Fn(..) => TyClass::Scalar,
        TyKind::Param(_) => TyClass::Scalar,
        _ => TyClass::Agg,
    }
}

/// 函数名编码 name_defid（防 Symbol 重名导致 LLVM 重定义）
pub fn fun_name(lc: &Lc, def: sem::DefId) -> String {
    format!(
        "@{}.{}",
        sanitize(lc.interner.resolve(lc.defs.get(def).name)),
        def.0
    )
}

/// Starry 标识符 → LLVM 全局名安全字符
pub fn sanitize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_alphanumeric() || c == '_' || c == '.' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    out
}

/// 浮点字面量：LLVM 位模式十六进制（float/double 均为 16 位模式文本）。
/// `is_f32` 时先把值舍入到 f32 再取位模式（f64 常量落在 float 槽的兼容路径）。
pub fn float_lit(v: f64, is_f32: bool) -> String {
    // LLVM floating hexadecimal constants carry a 64-bit pattern even when
    // the instruction type is `float`; clang validates this on LLVM 22.
    let bits = if is_f32 {
        (v as f32 as f64).to_bits()
    } else {
        v.to_bits()
    };
    format!("0x{bits:016X}")
}
