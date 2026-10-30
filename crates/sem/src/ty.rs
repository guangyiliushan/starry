//! 类型 arena：结构去重 + 提升格
//!
//! v0.3 两类类型：固定型需显式 `as`，自动提升型按格提升（龙书 6.5
//! 的类型检查框架；Kotlin 风格整型加宽教学化简，见 semantic-notes）。
//! `Error` 为投毒类型：出错子树统一为 Error，抑制级联诊断。

use std::collections::BTreeMap;

use crate::def::DefId;

/// 类型 ID（arena 索引；`Copy`，跨 pass 稳定）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TyId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum IntKind {
    I4,
    I8,
    I16,
    I32,
    I64,
    I128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FloatKind {
    F32,
    F64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TyKind {
    Int(IntKind),
    Float(FloatKind),
    Bool,
    Char,
    String,
    Unit,
    Null,
    /// 命名类型（泛型实参已代入）；DefId 指向 struct/enum/class/alias/builtin
    Named(DefId, Vec<TyId>),
    Nullable(TyId),
    Array(TyId, u32),
    Tuple(Vec<TyId>),
    Range(TyId),
    Fn(Vec<TyId>, TyId),
    /// 类型对象（类型名在值位置出现）：仅允许作构造器 callee，
    /// 其余使用点一律诊断（见 semantic-notes「类型名与值分立」）
    TypeObject(DefId),
    /// 「类型尚不可推定」哨兵（如顶层绑定依赖顺序）：不投毒，使用点照常诊断。
    /// 与 `Error`（真错误、投毒抑制级联）区分——见 semantic-notes。
    Unknown,
    /// 投毒类型
    Error,
}

/// 类型 arena（结构去重：相等类型只存一份）
#[derive(Debug)]
pub struct TyArena {
    kinds: Vec<TyKind>,
    dedup: BTreeMap<TyKind, TyId>,
    named_cache: BTreeMap<DefId, TyId>,
    pub unit: TyId,
    pub bool_: TyId,
    pub string: TyId,
    pub char_: TyId,
    pub null: TyId,
    pub unknown: TyId,
    pub error: TyId,
    pub default_int: TyId,
    pub default_float: TyId,
}

impl Default for TyArena {
    fn default() -> Self {
        Self::new()
    }
}

fn int_rank(k: IntKind) -> u8 {
    match k {
        IntKind::I4 => 0,
        IntKind::I8 => 1,
        IntKind::I16 => 2,
        IntKind::I32 => 3,
        IntKind::I64 => 4,
        IntKind::I128 => 5,
    }
}

impl TyArena {
    pub fn new() -> Self {
        let mut arena = Self {
            kinds: Vec::new(),
            dedup: BTreeMap::new(),
            named_cache: BTreeMap::new(),
            unit: TyId(0),
            bool_: TyId(0),
            string: TyId(0),
            char_: TyId(0),
            null: TyId(0),
            unknown: TyId(0),
            error: TyId(0),
            default_int: TyId(0),
            default_float: TyId(0),
        };
        arena.unit = arena.intern(TyKind::Unit);
        arena.bool_ = arena.intern(TyKind::Bool);
        arena.string = arena.intern(TyKind::String);
        arena.char_ = arena.intern(TyKind::Char);
        arena.null = arena.intern(TyKind::Null);
        arena.unknown = arena.intern(TyKind::Unknown);
        arena.error = arena.intern(TyKind::Error);
        arena.default_int = arena.intern(TyKind::Int(IntKind::I32));
        arena.default_float = arena.intern(TyKind::Float(FloatKind::F64));
        arena
    }

    /// 结构去重入池；返回的 TyId 跨 pass 稳定（IR 阶段构造类型用同一入口）
    pub fn intern(&mut self, kind: TyKind) -> TyId {
        if let Some(&id) = self.dedup.get(&kind) {
            return id;
        }
        let id = TyId(self.kinds.len() as u32);
        self.kinds.push(kind.clone());
        self.dedup.insert(kind, id);
        id
    }

    pub fn kind(&self, id: TyId) -> &TyKind {
        &self.kinds[id.0 as usize]
    }

    /// builtin 命名类型（无泛型实参）的缓存 Named 类型
    pub fn builtin_named(&mut self, def: DefId) -> TyId {
        if let Some(&id) = self.named_cache.get(&def) {
            return id;
        }
        let id = self.intern(TyKind::Named(def, Vec::new()));
        self.named_cache.insert(def, id);
        id
    }

    pub fn nullable(&mut self, inner: TyId) -> TyId {
        self.intern(TyKind::Nullable(inner))
    }

    pub fn is_error(&self, id: TyId) -> bool {
        matches!(self.kind(id), TyKind::Error)
    }

    pub fn is_unknown(&self, id: TyId) -> bool {
        matches!(self.kind(id), TyKind::Unknown)
    }

    /// 「尚未推定」哨兵判定：绑定注解槽的默认值既可能是 Unknown（未推定）
    /// 也可能是 Error（推定失败）；两者都需要被推断值覆盖
    pub fn needs_infer(&self, id: TyId) -> bool {
        self.is_error(id) || self.is_unknown(id)
    }

    /// 可序比较（< 族）限定：数值与字符；等值比较仍按 join 判定
    pub fn is_orderable(&self, id: TyId) -> bool {
        matches!(
            self.kind(id),
            TyKind::Int(_) | TyKind::Float(_) | TyKind::Char
        )
    }

    fn is_integral_kind(&self, id: TyId) -> bool {
        matches!(self.kind(id), TyKind::Int(_))
    }

    pub fn is_numeric(&self, id: TyId) -> bool {
        matches!(self.kind(id), TyKind::Int(_) | TyKind::Float(_))
    }

    /// 提升格：`from` 可隐式赋值给 `to`
    pub fn assignable(&self, from: TyId, to: TyId) -> bool {
        if from == to {
            return true;
        }
        // 可空目标：Null 与任意 T 均可提升入 Nullable(T)（逐层剥离，变度不变）
        if let TyKind::Nullable(inner) = self.kind(to) {
            let inner = *inner;
            return matches!(self.kind(from), TyKind::Null) || self.assignable(from, inner);
        }
        match (self.kind(from), self.kind(to)) {
            // Unknown 不投毒：不可指派（使用点应已诊断）
            (TyKind::Unknown, TyKind::Unknown) => from == to,
            (TyKind::Unknown, _) | (_, TyKind::Unknown) => false,
            (TyKind::Error, _) | (_, TyKind::Error) => true,
            (TyKind::Int(f), TyKind::Int(t)) => int_rank(*f) <= int_rank(*t),
            (TyKind::Int(_), TyKind::Float(_)) => true,
            (TyKind::Float(f), TyKind::Float(t)) => f <= t,
            (TyKind::Named(fd, fa), TyKind::Named(td, ta)) => fd == td && fa == ta,
            // 聚合逐元素走格（对齐 Tuple 臂）：`[int; 3] = [1, 2, 3]` 经 i32→int 加宽成立
            (TyKind::Array(fe, fl), TyKind::Array(te, tl)) => fl == tl && self.assignable(*fe, *te),
            (TyKind::Range(f), TyKind::Range(t)) => self.assignable(*f, *t),
            (TyKind::Tuple(fs), TyKind::Tuple(ts)) => {
                fs.len() == ts.len() && fs.iter().zip(ts).all(|(f, t)| self.assignable(*f, *t))
            }
            _ => false,
        }
    }

    /// 汇合：二元运算 / 分支汇合的公共类型；`None` = 不可比（调用方诊断）
    pub fn join(&mut self, a: TyId, b: TyId) -> Option<TyId> {
        // 类型对象不与任何类型汇合（先于相等短路，否则 [P, P] 漏网）
        if matches!(self.kind(a), TyKind::TypeObject(_))
            || matches!(self.kind(b), TyKind::TypeObject(_))
        {
            return None;
        }
        if a == b {
            return Some(a);
        }
        let ka = self.kind(a).clone();
        let kb = self.kind(b).clone();
        match (&ka, &kb) {
            (TyKind::Error, _) => Some(a),
            (_, TyKind::Error) => Some(b),
            (TyKind::Unknown, TyKind::Unknown) => Some(a),
            (TyKind::Unknown, _) | (_, TyKind::Unknown) => None,
            (TyKind::Null, TyKind::Nullable(_)) => Some(b),
            (TyKind::Nullable(_), TyKind::Null) => Some(a),
            // null ∨ T = T?；Nullable(S) ∨ T = Nullable(S ∨ T)
            (TyKind::Null, _) => Some(self.nullable(b)),
            (_, TyKind::Null) => Some(self.nullable(a)),
            (TyKind::Nullable(x), _) => self.join(*x, b).map(|j| self.nullable(j)),
            (_, TyKind::Nullable(y)) => self.join(a, *y).map(|j| self.nullable(j)),
            (TyKind::Int(f), TyKind::Int(t)) => {
                let rank = int_rank(*f).max(int_rank(*t));
                let kind = match rank {
                    0 => IntKind::I4,
                    1 => IntKind::I8,
                    2 => IntKind::I16,
                    3 => IntKind::I32,
                    4 => IntKind::I64,
                    _ => IntKind::I128,
                };
                Some(self.intern(TyKind::Int(kind)))
            }
            (TyKind::Float(_), TyKind::Float(_)) => {
                Some(self.intern(TyKind::Float(FloatKind::F64)))
            }
            (TyKind::Int(_), TyKind::Float(t)) => Some(self.intern(TyKind::Float(*t))),
            (TyKind::Float(f), TyKind::Int(_)) => Some(self.intern(TyKind::Float(*f))),
            (TyKind::Named(fd, fa), TyKind::Named(td, ta)) if fd == td && fa == ta => Some(a),
            // 数组/区间逐元素汇合（长度不变）
            (TyKind::Array(fe, fl), TyKind::Array(te, tl)) if fl == tl => {
                let j = self.join(*fe, *te)?;
                Some(self.intern(TyKind::Array(j, *fl)))
            }
            (TyKind::Range(f), TyKind::Range(t)) => {
                let j = self.join(*f, *t)?;
                Some(self.intern(TyKind::Range(j)))
            }
            (TyKind::Tuple(fs), TyKind::Tuple(ts)) if fs.len() == ts.len() => {
                let mut out = Vec::with_capacity(fs.len());
                for (f, t) in fs.iter().zip(ts) {
                    out.push(self.join(*f, *t)?);
                }
                Some(self.intern(TyKind::Tuple(out)))
            }
            _ => None,
        }
    }

    /// 便捷：整型字面量默认类型核查（测试用）
    pub fn is_integral(&self, id: TyId) -> bool {
        self.is_integral_kind(id)
    }
}
