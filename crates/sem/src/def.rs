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
    LocalBinding {
        mutable: bool,
        /// M4 slice 3：`weak var` 弱引用绑定（破循环；读取/重赋值 slice 4）
        weak: bool,
    },
    Param,
    Struct,
    /// M3 字段可变性三态：Some(true)=var 字段（恒可写）/
    /// Some(false)=val 字段（恒不可写）/None=legacy（按根绑定可变性）。
    StructField {
        parent: DefId,
        index: usize,
        mutable: Option<bool>,
    },
    Enum,
    EnumVariant {
        parent: DefId,
        index: usize,
    },
    Class,
    TypeAlias,
    /// M7 泛型类型形参：un f<T>(...) 的 T。注册进泛型作用域；
    /// lower 期经 SemState.mono_subst 兑现为具体类型（克隆体）或
    /// TyKind::Param(index)（泛型原件签名）。
    TypeParam {
        index: u32,
    },
    BuiltinType,
    /// v1 I/O builtin `print(value)`; overload by argument type at call sites.
    BuiltinPrint,
    ImportNs {
        segments: Vec<Symbol>,
    },
    /// M5 slice 2a: package namespace（member_scope = 包的顶层作用域；
    /// 注册进父包/根，expand_import 首段从根解析）
    PackageNs,
    /// M9 9a：trait 声明（member_scope = 方法签名 defs；仅静态分派，
    /// 无默认实现/关联类型/泛型 trait——v1 拒收面见 declare）
    Trait,
}

/// M9 9a：impl 注册表条目（静态分派解析用；9b 消费）
#[derive(Debug, Clone)]
pub struct TraitImpl {
    pub trait_def: DefId,
    pub type_def: DefId,
    /// impl 声明 span（resolve/check 按 AST 项回找注册表条目）
    pub span: Span,
    /// 方法名 → impl 方法 def（签名已与 trait 核对一致）
    pub methods: Vec<(Symbol, DefId)>,
}

/// M5 slice 3：顶层声明可见性（Kotlin 式；protected 为 class 成员专用，
/// 顶层使用稳定拒收）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Vis {
    /// 默认：包内可见 + 跨包可 import
    Pub,
    /// 包私有：仅同包可见（绑定包身份，D-009）
    Internal,
    /// 文件私有：仅同段（文件）可见
    Private,
}

/// 定义签名槽（各 kind 按需取用；单一结构避免按 kind 分型的抽象膨胀）
#[derive(Debug, Clone)]
pub struct DefSig {
    /// M5 slice 3：顶层声明可见性（默认 Pub）
    pub vis: Vis,
    /// M7 slice 2：类型形参默认值（declare 期在类型形参作用域 lower；
    /// 与 type_params 对齐；未提供的为 None）
    pub tp_defaults: Vec<Option<TyId>>,
    /// M9 9c：类型形参 bounds——每项 = 该形参的 trait def 列表
    ///（`T : A & B` 交集；实例化时逐一查 impl 注册表兑现）
    pub tp_bounds: Vec<Vec<DefId>>,
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
            vis: Vis::Pub,
            tp_defaults: Vec::new(),
            tp_bounds: Vec::new(),
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
    /// extern "c" 声明：调用点按外部符号名发射，不生成函数体
    pub external: bool,
    /// M7：类型形参个数（0 = 非泛型）；check 的 type_call 据此刻度实参并代入
    pub ty_params: u32,
    /// 声明在 SourceFile.items 中的下标（M7 克隆 def 反查声明用；
    /// 非顶层声明 / 实例化 spec def 无 items 对应 = None）
    pub decl: Option<usize>,
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

    /// M9 9a：作用域内全部名→def（impl 覆盖核对用）
    pub fn entries(&self, scope: ScopeId) -> Vec<(Symbol, DefId)> {
        self.scopes[scope.0 as usize]
            .names
            .iter()
            .map(|(n, d)| (*n, *d))
            .collect()
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
