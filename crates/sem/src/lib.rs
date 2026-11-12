//! Starry 语义分析：三遍 pass + 明确赋值 + 最小线性 move 检查
//!
//! ```text
//! ParseOutput ──→ declare（作用域树 + defs + 顶层签名 lower）
//!             ──→ resolve（名字 → DefId；label/flow；pattern 构造器）
//!             ──→ check（自底向上定型 → expr_ty；Error 投毒抑制级联）
//!             ─→ definite（结构化明确赋值）
//!             ──→ moveck（Own 值线性移动检查，join 点保守并集）
//!             ──→ SemanticOutput（AST 原样 + arena + 侧表）
//! ```
//!
//! 设计决策记录见 `docs/syntax/semantic-notes.md`（与 parser-notes 同位）。
//! 核心约定：语义数据零内嵌 AST，全部走 `SideTable`（NodeId 索引，
//! 容量 = ParseOutput.node_count，O(1)）；`Error` 类型/定义投毒，
//! 出错子树不再级联诊断。

mod check;
mod declare;
mod definite;
mod mono;
mod moveck;
mod resolve;

pub mod def;
pub mod ty;

use ast::interner::Interner;
use ast::syntax::ParseOutput;
use ast::syntax::node::{DiagCollector, Diagnostic, NodeId, SourceFile, Span};

pub use def::{Def, DefArena, DefId, DefKind, DefSig, ScopeArena, ScopeId, TraitImpl};
pub use ty::{FloatKind, IntKind, TyArena, TyId, TyKind};

/// break/continue/return 的已解析目标
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowKind {
    Break,
    Continue,
    Return,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowRes {
    pub target: NodeId,
    pub kind: FlowKind,
}

/// 构造器模式的已解析目标（variant = Some 表示枚举变体序号）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PatRes {
    pub ctor: DefId,
    pub variant: Option<usize>,
}

/// 稠密侧表：NodeId 索引，越界返回 None（受控，不 panic）
#[derive(Debug, Clone)]
pub struct SideTable<T>(Vec<Option<T>>);

impl<T: Clone> SideTable<T> {
    pub fn new(node_count: u32) -> Self {
        Self(vec![None; node_count as usize])
    }

    /// M7 克隆追加后扩容（NodeId 越过原始 node_count；保持密度契约）
    pub fn grow(&mut self, new_len: usize) {
        if new_len > self.0.len() {
            self.0.resize(new_len, None);
        }
    }

    pub fn set(&mut self, id: NodeId, value: T) {
        // node_count 记账漂移必须是可观测的不变量，而非静默丢语义
        debug_assert!(
            (id.0 as usize) < self.0.len(),
            "side-table set out of range: node id beyond node_count"
        );
        if let Some(slot) = self.0.get_mut(id.0 as usize) {
            *slot = Some(value);
        }
    }

    pub fn get(&self, id: NodeId) -> Option<&T> {
        self.0.get(id.0 as usize).and_then(|slot| slot.as_ref())
    }
}

/// S2/P2-3：捕获表条目（名, ty），首现序
pub type LambdaCaps = Vec<(ast::interner::Symbol, TyId)>;
/// S2/P2-3：捕获收集栈元素（lambda node, 捕获作用域, 捕获表）
pub(crate) type CapFrame = (u32, def::ScopeId, LambdaCaps);

/// 语义分析产物（IR 生成阶段的输入契约）
pub struct SemanticOutput {
    pub source: SourceFile,
    pub interner: Interner,
    pub diagnostics: Vec<Diagnostic>,
    pub tys: TyArena,
    pub defs: DefArena,
    pub expr_ty: SideTable<TyId>,
    pub expr_def: SideTable<DefId>,
    pub pat_res: SideTable<PatRes>,
    pub flow_res: SideTable<FlowRes>,
    /// 绑定模式 → LocalBinding DefId（irc 按 DefId 取 sig.ty 与 mutability）
    pub pat_binding_def: SideTable<DefId>,
    /// 类型注解节点 → 定型结果（irc 取模式标注的 widening 目标）
    pub ty_of_node: SideTable<TyId>,
    /// M7：顶层声明下标 → def（irc 按 item 找 def；克隆 def 与原件同 span，
    /// span 反查会撞车）
    pub decl_def: Vec<Option<DefId>>,
    /// M4 slice 2b：作用域 arena 只读导出——irc 的 deinit/成员查找需要
    /// member_scope 查名（SemanticOutput 整体消费期有效）
    pub scopes: ScopeArena,
    /// M9 9b：impl 注册表导出（irc 的 impl 方法体 lowering 用）
    pub impls: Vec<def::TraitImpl>,
    pub lambda_defs: std::collections::BTreeMap<u32, DefId>,
    /// S2/P2-3：lambda expr id → 捕获表（名称按首现序）
    pub lambda_caps: std::collections::BTreeMap<u32, Vec<(ast::interner::Symbol, TyId)>>,
}

/// 语义分析入口；parser 诊断原样保留在前，语义诊断附加在后
pub fn analyze(parse: ParseOutput) -> SemanticOutput {
    let mut sem = Sem::new(parse);
    {
        let ast = &sem.ast;
        let st = &mut sem.st;
        declare::run(ast, st);
        resolve::run(ast, st);
        check::run(ast, st);
        st.items_done = ast.items.len();
    }
    // M7 泛型实例化波次：check 排队的 fun 实例在此克隆追加并增量过三遍；
    // 新发现的实例化进下一波，直到不动点
    mono::run_waves(&mut sem);
    {
        let ast = &sem.ast;
        let st = &mut sem.st;
        definite::run(ast, st);
        moveck::run(ast, st);
    }
    sem.finish()
}

/// 共享可变状态（与 AST 分离，三遍 pass 以 `(&SourceFile, &mut SemState)` 签名并行借用）
pub(crate) struct SemState {
    pub interner: Interner,
    /// 源码文本（span → 字面量值解码）
    pub text: String,
    pub diags: DiagCollector,
    pub tys: TyArena,
    pub defs: DefArena,
    pub scopes: ScopeArena,
    pub expr_ty: SideTable<TyId>,
    pub expr_def: SideTable<DefId>,
    pub pat_res: SideTable<PatRes>,
    pub flow_res: SideTable<FlowRes>,
    pub node_scope: SideTable<ScopeId>,
    pub ty_of_node: SideTable<TyId>,
    pub pat_binding_def: SideTable<DefId>,
    // resolve/check 上下文
    pub labels: Vec<LabelFrame>,
    pub funs: Vec<FunFrame>,
    pub loops: Vec<LoopFrame>,
    pub own_def: Option<DefId>,
    /// M5 slice 3：包作用域标记（pkg_scope 创建时记录；可见性校验
    /// 用它判定两个作用域是否同属一个包身份链）
    pub pkg_scopes: Vec<ScopeId>,
    /// M9 9a：impl 注册表（静态分派按 (trait, type) 查；重复 impl 拒绝）
    pub impls: Vec<crate::def::TraitImpl>,
    /// M9 9a：impl 头解析期置位——lower_ty 对 trait 名的拒收
    /// 跳过（impl Show for P 的 Show 不是 trait 对象标注）
    pub in_impl_header: bool,
    /// M6：unsafe 块深度——extern "c" 调用要求在 unsafe 上下文中
    ///（language-design：FFI 调用在 unsafe 中）
    pub unsafe_depth: usize,
    /// M8: lambda expected Fn type stack
    pub lambda_exp: Vec<Option<TyId>>,
    /// M8: lambda expr id -> fun def
    pub lambda_defs: std::collections::BTreeMap<u32, DefId>,
    /// S2/P2-3：捕获收集栈（node, lambda 作用域, 捕获表）
    pub lambda_cap_stack: Vec<CapFrame>,
    /// S2/P2-3：lambda expr id → 捕获表（pop 时落账）
    pub lambda_caps: std::collections::BTreeMap<u32, LambdaCaps>,
    // ---- M7 泛型单态化（generic-mono-spike.md） ----
    /// fun 实例化作业队列（struct 同步完成，不排队）
    pub mono_queue: Vec<mono::MonoJob>,
    /// 实例化注册表：(泛型 def, 具体类型实参) → spec def；去重靠它
    pub mono_insts: std::collections::BTreeMap<(DefId, Vec<TyId>), DefId>,
    /// 当前波次的类型形参兑现表：TypeParam def → 具体 TyId
    pub mono_subst: Vec<(DefId, TyId)>,
    /// 克隆 NodeId 分配的下一基址（从原始 node_count 起）
    pub next_node: u32,
    /// 增量 pass 已处理的 items 前缀长度
    pub items_done: usize,
    /// 顶层声明下标 → def（M7 克隆 def 与原件同 span，span 反查会撞车）
    pub decl_def: Vec<Option<DefId>>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LabelFrame {
    pub label: Option<ast::interner::Symbol>,
    pub node: NodeId,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FunFrame {
    pub node: NodeId,
    pub def: DefId,
}

#[derive(Debug)]
pub(crate) struct LoopFrame {
    pub break_values: Vec<TyId>,
    pub break_total: usize,
    pub break_with_value: usize,
}

pub(crate) struct Sem {
    ast: SourceFile,
    st: SemState,
}

impl Sem {
    fn new(parse: ParseOutput) -> Self {
        let mut diags = DiagCollector::new();
        for diag in parse.diagnostics {
            diags.emit(diag);
        }
        let node_count = parse.node_count;
        let text = parse.text;
        let mut st = SemState {
            interner: parse.interner,
            text,
            diags,
            tys: TyArena::new(),
            defs: DefArena::new(),
            scopes: ScopeArena::new(),
            expr_ty: SideTable::new(node_count),
            expr_def: SideTable::new(node_count),
            pat_res: SideTable::new(node_count),
            flow_res: SideTable::new(node_count),
            node_scope: SideTable::new(node_count),
            ty_of_node: SideTable::new(node_count),
            pat_binding_def: SideTable::new(node_count),
            labels: Vec::new(),
            funs: Vec::new(),
            loops: Vec::new(),
            own_def: None,
            pkg_scopes: Vec::new(),
            impls: Vec::new(),
            lambda_defs: std::collections::BTreeMap::new(),
            lambda_cap_stack: Vec::new(),
            lambda_caps: std::collections::BTreeMap::new(),
            mono_queue: Vec::new(),
            in_impl_header: false,
            unsafe_depth: 0,
            lambda_exp: Vec::new(),
            mono_insts: std::collections::BTreeMap::new(),
            mono_subst: Vec::new(),
            next_node: node_count,
            items_done: 0,
            decl_def: Vec::new(),
        };
        // v1 I/O builtin：注册到根作用域；调用点按实参类型实例化 runtime 入口。
        let root = st.scopes.root();
        let print = st.interner.intern("print");
        let def = st.defs.push(Def {
            kind: DefKind::BuiltinPrint,
            name: print,
            scope: root,
            span: Span { start: 0, end: 0 },
            sig: DefSig::new(st.tys.unit, st.tys.unknown),
            resolving: false,
            alias_of: None,
            alias_resolved: None,
            member_scope: None,
            external: false,
            ty_params: 0,
            decl: None,
        });
        st.scopes.insert(root, print, def);
        Self { ast: parse.ast, st }
    }

    fn finish(self) -> SemanticOutput {
        SemanticOutput {
            source: self.ast,
            interner: self.st.interner,
            diagnostics: self.st.diags.all().to_vec(),
            tys: self.st.tys,
            defs: self.st.defs,
            expr_ty: self.st.expr_ty,
            expr_def: self.st.expr_def,
            pat_res: self.st.pat_res,
            flow_res: self.st.flow_res,
            pat_binding_def: self.st.pat_binding_def,
            ty_of_node: self.st.ty_of_node,
            decl_def: self.st.decl_def,
            scopes: self.st.scopes,
            impls: self.st.impls,
            lambda_defs: self.st.lambda_defs,
            lambda_caps: self.st.lambda_caps,
        }
    }
}
