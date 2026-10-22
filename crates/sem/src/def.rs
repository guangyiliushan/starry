//! 定义与作用域 arena：DefId / ScopeId 索引，全 `Copy`，零引用零克隆
//!
//! 作用域符号表用 `BTreeMap`（starry-compiler 确定性门：不依赖迭代序
//! 之外的任何顺序保证；查找 O(log n) 在编译期规模无感）。

use std::collections::BTreeMap;

use ast::syntax::node::{Span, Symbol};
use ast::syntax::ty::TypeWrap;

use crate::ty::TyId;

/// 定义 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DefId(pub u32);

/// 作用域 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ScopeId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefKind {
    Fun,
    LocalBinding { mutable: bool },
    Param,
    Struct,
    StructField { parent: DefId, index: usize },
    Enum,
    EnumVariant { parent: DefId, index: usize },
    Class,
    TypeAlias,
    BuiltinType,
    ImportNs { segments: Vec<Symbol> },
}

/// 定义签名槽（各 kind 按需取用；单一结构避免按 kind 分型的抽象膨胀）
#[derive(Debug, Clone)]
pub struct DefSig {
    pub params: Vec<(Symbol, TyId)>,
    pub ret: TyId,
    pub ty: TyId,
    /// 枚举变体：名 + 形参（名, 类型）——名保留以支持命名实参构造
    pub variants: Vec<(Symbol, Vec<(Symbol, TyId)>)>,
    pub fields: Vec<(Symbol, TyId)>,
    /// 参数默认值标记（与 params 对齐）
    pub param_defaults: Vec<bool>,
}

impl DefSig {
    pub fn new(unit: TyId, error: TyId) -> Self {
        Self {
            params: Vec::new(),
            ret: unit,
            ty: error,
            variants: Vec::new(),
            fields: Vec::new(),
            param_defaults: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Def {
    pub kind: DefKind,
    pub name: Symbol,
    pub scope: ScopeId,
    pub span: Span,
    pub sig: DefSig,
    /// 别名循环守卫（lower 期间为 true）
    pub resolving: bool,
    /// 类型别名目标（lower 用）
    pub alias_of: Option<TypeWrap>,
    /// 别名目标解析出的命名定义（alias_target 成功且结果为 Named 时写入；
    /// 值位置的别名经此走到目标的构造器分支）
    pub alias_resolved: Option<DefId>,
    /// 成员作用域（struct/enum/class）
    pub member_scope: Option<ScopeId>,
}

#[derive(Debug, Default)]
pub struct DefArena {
    defs: Vec<Def>,
}

impl DefArena {
    pub fn new() -> Self {
        Self { defs: Vec::new() }
    }

    pub fn push(&mut self, def: Def) -> DefId {
        let id = DefId(self.defs.len() as u32);
        self.defs.push(def);
        id
    }

    pub fn get(&self, id: DefId) -> &Def {
        &self.defs[id.0 as usize]
    }

    pub fn get_mut(&mut self, id: DefId) -> &mut Def {
        &mut self.defs[id.0 as usize]
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }
}

#[derive(Debug)]
pub struct ScopeData {
    pub parent: Option<ScopeId>,
    pub names: BTreeMap<Symbol, DefId>,
}

#[derive(Debug)]
pub struct ScopeArena {
    scopes: Vec<ScopeData>,
}

impl ScopeArena {
    /// 新建 arena，自动创建根作用域 0
    pub fn new() -> Self {
        Self {
            scopes: vec![ScopeData {
                parent: None,
                names: BTreeMap::new(),
            }],
        }
    }

    pub fn root(&self) -> ScopeId {
        ScopeId(0)
    }

    pub fn new_scope(&mut self, parent: Option<ScopeId>) -> ScopeId {
        let id = ScopeId(self.scopes.len() as u32);
        self.scopes.push(ScopeData {
            parent,
            names: BTreeMap::new(),
        });
        id
    }

    /// 插入；返回被顶掉的旧定义（同名重复 → 调用方诊断 SemDuplicateDef）
    pub fn insert(&mut self, scope: ScopeId, name: Symbol, def: DefId) -> Option<DefId> {
        self.scopes[scope.0 as usize].names.insert(name, def)
    }

    pub fn lookup_local(&self, scope: ScopeId, name: Symbol) -> Option<DefId> {
        self.scopes[scope.0 as usize].names.get(&name).copied()
    }

    pub fn parent_of(&self, scope: ScopeId) -> Option<ScopeId> {
        self.scopes[scope.0 as usize].parent
    }

    /// 沿父链查找
    pub fn lookup(&self, scope: ScopeId, name: Symbol) -> Option<DefId> {
        let mut cur = Some(scope);
        while let Some(s) = cur {
            if let Some(&def) = self.scopes[s.0 as usize].names.get(&name) {
                return Some(def);
            }
            cur = self.scopes[s.0 as usize].parent;
        }
        None
    }
}

impl Default for ScopeArena {
    fn default() -> Self {
        Self::new()
    }
}
