//! LLVM 类型映射。
//!
//! 槽模型的关键简化（2026-10-27 llvme 落地时钉死）：**一切聚合值的
//! LLVM 类型都是 `[n x i64]`**（n = scalar_slots）。tag/flag 是槽 0
//! 里的 i8（写时就地 bitcast），payload 自槽 1——记录型与和型的槽
//! 寻址由此完全统一（slot i = gep [n x i64], p, 0, i），无命名类型
//! 定义，结构等价的类型在 LLVM 侧天然可互换。

use std::fmt::Write as _;

use ast::interner::Interner;
use irc::layout::LayoutView;
use sem::{FloatKind, IntKind, TyArena, TyId, TyKind};

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
pub fn scalar_ty(tys: &TyArena, ty: TyId) -> String {
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
        TyKind::Unit => "void".into(),
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

pub fn class_of(tys: &TyArena, ty: TyId) -> TyClass {
    match tys.kind(ty) {
        TyKind::Int(_)
        | TyKind::Float(_)
        | TyKind::Bool
        | TyKind::Char
        | TyKind::String
        | TyKind::Unit => TyClass::Scalar,
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

/// 浮点字面量：十六进制（位精确往返，llvm-as 层抓违例）。
/// `is_f32` 时按 f32 位模式格式化（f64 常量落在 float 槽的兼容路径）
pub fn float_lit(v: f64, is_f32: bool) -> String {
    if is_f32 {
        let f = v as f32;
        if f.is_nan() {
            return "0x7FC00000".into();
        }
        if f.is_infinite() {
            return if f < 0.0 {
                "-0x7F800000".into()
            } else {
                "0x7F800000".into()
            };
        }
        if f == 0.0 {
            return if f.is_sign_negative() {
                "-0.0".into()
            } else {
                "0.0".into()
            };
        }
        let bits = f.to_bits();
        let sign = bits >> 31;
        let exp = ((bits >> 23) & 0xff) as i64 - 127;
        let man = bits & 0x7f_ffff;
        let mut s = String::new();
        if sign == 1 {
            s.push('-');
        }
        s.push_str("0x1");
        if man != 0 {
            let mut hex = format!("{:06X}", man);
            while hex.ends_with('0') {
                hex.pop();
            }
            s.push('.');
            s.push_str(&hex);
        }
        let _ = write!(s, "p{exp}");
        return s;
    }
    if v.is_nan() {
        return "0x7FF8000000000000".into();
    }
    if v.is_infinite() {
        return if v < 0.0 {
            "-0x7FF0000000000000".into()
        } else {
            "0x7FF0000000000000".into()
        };
    }
    if v == 0.0 {
        return if v.is_sign_negative() {
            "-0.0".into()
        } else {
            "0.0".into()
        };
    }
    let bits = v.to_bits();
    let sign = bits >> 63;
    let exp = ((bits >> 52) & 0x7ff) as i64 - 1023;
    let man = bits & 0xf_ffff_ffff_ffff;
    let mut s = String::new();
    if sign == 1 {
        s.push('-');
    }
    s.push_str("0x1");
    if man != 0 {
        let mut hex = format!("{:013X}", man);
        while hex.ends_with('0') {
            hex.pop();
        }
        s.push('.');
        s.push_str(&hex);
    }
    let _ = write!(s, "p{exp}");
    s
}
