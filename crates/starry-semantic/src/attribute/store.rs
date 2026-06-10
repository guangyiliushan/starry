use crate::attribute::context::WarmupStats;
use crate::attribute::result::AttrResult;
use crate::ir::operand::Operand;
use crate::ir::tac::TacInstr;
use crate::ty::Ty;
use starry_ast::{AstNode, LiteralValue};

/// 节点 ID（遍历顺序索引）
///
/// 在 DFS 遍历 AST 时按访问顺序分配，用于索引 Side Tables。
/// 这是一种非侵入式设计：AST 节点本身不需要携带 ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct NodeId(pub usize);

impl NodeId {
    pub fn get(&self) -> usize {
        self.0
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// 单个节点的属性集合
#[derive(Debug, Clone, Default)]
pub struct NodeAttrs {
    /// 表达式的静态类型
    pub ty: Option<Ty>,
    /// 是否为编译时常量
    pub is_const: bool,
    /// 常量折叠后的值
    pub const_value: Option<LiteralValue>,
    /// 是否为左值
    pub is_lvalue: bool,
    /// IR 生成阶段：结果存放位置
    pub place: Option<Operand>,
    /// IR 生成阶段：产出的 TAC 指令
    pub instructions: Vec<TacInstr>,
    /// 作用域 ID
    pub scope_id: Option<crate::attribute::env::ScopeId>,
    /// 符号名称（仅 Ident 节点）
    pub symbol_name: Option<String>,
    /// 节点深度（用于调试）
    pub depth: usize,
}

impl NodeAttrs {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_attr_result(result: &AttrResult) -> Self {
        Self {
            ty: Some(result.ty.clone()),
            is_const: result.is_const,
            const_value: result.const_value.clone(),
            is_lvalue: result.is_lvalue,
            place: result.place.clone(),
            instructions: result.instructions.clone(),
            ..Default::default()
        }
    }

    /// 合并另一个属性集
    pub fn merge(&mut self, other: &NodeAttrs) {
        if other.ty.is_some() {
            self.ty = other.ty.clone();
        }
        if other.is_const {
            self.is_const = true;
        }
        if other.const_value.is_some() {
            self.const_value = other.const_value.clone();
        }
        if other.is_lvalue {
            self.is_lvalue = true;
        }
        if other.place.is_some() {
            self.place = other.place.clone();
        }
        if !other.instructions.is_empty() {
            self.instructions.extend(other.instructions.iter().cloned());
        }
        if other.scope_id.is_some() {
            self.scope_id = other.scope_id;
        }
        if other.symbol_name.is_some() {
            self.symbol_name = other.symbol_name.clone();
        }
        if other.depth > 0 {
            self.depth = other.depth;
        }
    }

    /// 格式化属性信息为字符串，标注继承属性与综合属性
    pub fn format(&self) -> String {
        let mut inherited = Vec::new();
        let mut synthesized = Vec::new();

        // 继承属性（自顶向下通过 AttrContext 传入，在 enter 阶段写入）
        if let Some(scope) = self.scope_id {
            inherited.push(format!("scope=#{}", scope.0));
        }

        // 综合属性（自底向上通过 AttrResult 计算，在 leave 阶段写入）
        if let Some(ty) = &self.ty {
            synthesized.push(format!("ty={:?}", ty));
        }
        if self.is_const {
            synthesized.push("const".to_string());
        }
        if let Some(v) = &self.const_value {
            synthesized.push(format!("value={:?}", v));
        }
        if self.is_lvalue {
            synthesized.push("lvalue".to_string());
        }
        if let Some(place) = &self.place {
            synthesized.push(format!("place={}", place));
        }
        if let Some(name) = &self.symbol_name {
            synthesized.push(format!("symbol={}", name));
        }
        if !self.instructions.is_empty() {
            synthesized.push(format!("ir={} instrs", self.instructions.len()));
        }

        let mut parts = Vec::new();
        if !inherited.is_empty() {
            parts.push(format!("[继承] {}", inherited.join(", ")));
        }
        if !synthesized.is_empty() {
            parts.push(format!("[综合] {}", synthesized.join(", ")));
        }

        if parts.is_empty() {
            String::new()
        } else {
            parts.join(" ")
        }
    }
}

/// 单遍分析的属性表
#[derive(Debug, Clone, Default)]
pub struct PassAttrs {
    /// 节点属性向量（索引 = NodeId）
    pub attrs: Vec<NodeAttrs>,
}

impl PassAttrs {
    pub fn new() -> Self {
        Self::default()
    }

    /// 预分配容量
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            attrs: Vec::with_capacity(capacity),
        }
    }

    /// 设置节点属性
    pub fn set(&mut self, node_id: NodeId, attrs: NodeAttrs) {
        if node_id.0 >= self.attrs.len() {
            self.attrs.resize(node_id.0 + 1, NodeAttrs::new());
        }
        self.attrs[node_id.0] = attrs;
    }

    /// 获取节点属性
    pub fn get(&self, node_id: NodeId) -> Option<&NodeAttrs> {
        self.attrs.get(node_id.0)
    }

    /// 获取可变引用
    pub fn get_mut(&mut self, node_id: NodeId) -> Option<&mut NodeAttrs> {
        self.attrs.get_mut(node_id.0)
    }

    /// 追加节点属性（按遍历顺序）
    pub fn push(&mut self, attrs: NodeAttrs) -> NodeId {
        let id = NodeId(self.attrs.len());
        self.attrs.push(attrs);
        id
    }

    /// 合并另一个属性表
    pub fn merge(&mut self, other: &PassAttrs) {
        for (i, attrs) in other.attrs.iter().enumerate() {
            if i < self.attrs.len() {
                self.attrs[i].merge(attrs);
            } else {
                self.attrs.push(attrs.clone());
            }
        }
    }

    /// 节点数量
    pub fn len(&self) -> usize {
        self.attrs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.attrs.is_empty()
    }

    /// 打印带属性的 AST 结构
    pub fn print_ast(&self, root: &AstNode) {
        self.print_ast_inner(root, "", true, &mut 0);
    }

    fn print_ast_inner(&self, node: &AstNode, prefix: &str, is_last: bool, node_id: &mut usize) {
        let branch = if is_last { "└── " } else { "├── " };
        let child_prefix = if is_last { "    " } else { "│   " };

        let current_id = NodeId(*node_id);
        let attr_info = if let Some(attr) = self.get(current_id) {
            attr.format()
        } else {
            "(无属性)".to_string()
        };

        println!("{}{}{} {} {}", prefix, branch, current_id, node.summary(), attr_info);

        *node_id += 1;

        let children: Vec<_> = node.children();
        for (i, child) in children.iter().enumerate() {
            let is_last_child = i == children.len() - 1;
            let new_prefix = format!("{}{}", prefix, child_prefix);
            self.print_ast_inner(child, &new_prefix, is_last_child, node_id);
        }
    }
}

/// 完整的属性存储（跨所有遍次）
#[derive(Debug, Clone)]
pub struct AttrStore {
    /// 预热遍属性
    pub warmup: PassAttrs,
    /// 名称解析遍属性
    pub name_resolution: PassAttrs,
    /// 类型检查遍属性
    pub type_check: PassAttrs,
    /// IR 生成遍属性
    pub ir_generation: PassAttrs,
    /// 预热统计
    pub warmup_stats: Option<WarmupStats>,
    /// 节点总数
    pub node_count: usize,
}

impl AttrStore {
    pub fn new() -> Self {
        Self {
            warmup: PassAttrs::new(),
            name_resolution: PassAttrs::new(),
            type_check: PassAttrs::new(),
            ir_generation: PassAttrs::new(),
            warmup_stats: None,
            node_count: 0,
        }
    }

    /// 获取指定遍次的属性表
    pub fn get_pass_attrs(&self, pass: crate::attribute::context::Pass) -> &PassAttrs {
        match pass {
            crate::attribute::context::Pass::Warmup => &self.warmup,
            crate::attribute::context::Pass::NameResolution => &self.name_resolution,
            crate::attribute::context::Pass::TypeChecking => &self.type_check,
            crate::attribute::context::Pass::IrGeneration => &self.ir_generation,
        }
    }

    /// 获取指定遍次的可变属性表
    pub fn get_pass_attrs_mut(&mut self, pass: crate::attribute::context::Pass) -> &mut PassAttrs {
        match pass {
            crate::attribute::context::Pass::Warmup => &mut self.warmup,
            crate::attribute::context::Pass::NameResolution => &mut self.name_resolution,
            crate::attribute::context::Pass::TypeChecking => &mut self.type_check,
            crate::attribute::context::Pass::IrGeneration => &mut self.ir_generation,
        }
    }

    /// 合并所有遍次的属性
    pub fn merged(&self) -> PassAttrs {
        let mut merged = PassAttrs::new();
        merged.merge(&self.warmup);
        merged.merge(&self.name_resolution);
        merged.merge(&self.type_check);
        merged.merge(&self.ir_generation);
        merged
    }
}

impl Default for AttrStore {
    fn default() -> Self {
        Self::new()
    }
}

/// AST 遍历器：为节点分配 NodeId 并收集属性
///
/// 这是非侵入式架构的核心：通过 DFS 遍历为每个节点分配连续的 NodeId，
/// 然后使用这些 ID 索引 Side Tables。
pub struct AstWalker {
    next_id: usize,
    depth: usize,
}

impl AstWalker {
    pub fn new() -> Self {
        Self {
            next_id: 0,
            depth: 0,
        }
    }

    /// 为节点分配 NodeId
    pub fn alloc_node_id(&mut self) -> NodeId {
        let id = NodeId(self.next_id);
        self.next_id += 1;
        id
    }

    /// 获取已分配的节点数量
    pub fn node_count(&self) -> usize {
        self.next_id
    }

    /// 遍历 AST，为每个节点分配 NodeId 并调用回调
    pub fn walk<F>(&mut self, node: &AstNode, mut callback: F)
    where
        F: FnMut(&AstNode, NodeId, usize),
    {
        self.walk_inner(node, &mut callback);
    }

    fn walk_inner<F>(&mut self, node: &AstNode, callback: &mut F)
    where
        F: FnMut(&AstNode, NodeId, usize),
    {
        let id = self.alloc_node_id();
        let depth = self.depth;
        
        callback(node, id, depth);
        
        self.depth += 1;
        for child in node.children() {
            self.walk_inner(child, callback);
        }
        self.depth -= 1;
    }

    /// 统计节点数量
    pub fn count_nodes(node: &AstNode) -> usize {
        let mut count = 0;
        Self::count_inner(node, &mut count);
        count
    }

    fn count_inner(node: &AstNode, count: &mut usize) {
        *count += 1;
        for child in node.children() {
            Self::count_inner(child, count);
        }
    }
}

impl Default for AstWalker {
    fn default() -> Self {
        Self::new()
    }
}
