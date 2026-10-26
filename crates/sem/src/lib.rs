//! Starry 语义分析：三遍 pass + 最小线性 move 检查（M3.5）
//!
//! ```text
//! ParseOutput ──→ declare（作用域树 + defs + 顶层签名 lower）
//!             ──→ resolve（名字 → DefId；label/flow；pattern 构造器）
//!             ──→ check（自底向上定型 → expr_ty；Error 投毒抑制级联）
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
mod moveck;
mod resolve;

pub mod def;
pub mod ty;

use ast::interner::Interner;
use ast::syntax::ParseOutput;
use ast::syntax::node::{DiagCollector, Diagnostic, NodeId, SourceFile};

pub use def::{Def, DefArena, DefId, DefKind, DefSig, ScopeArena, ScopeId};
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
        moveck::run(ast, st);
    }
    sem.finish()
}

/// 共享可变状态（与 AST 分离，三遍 pass 以 `(&SourceFile, &mut SemState)` 签名并行借用）
pub(crate) struct SemState {
    pub interner: Interner,
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
        Self {
            ast: parse.ast,
            st: SemState {
                interner: parse.interner,
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
            },
        }
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
        }
    }
}
