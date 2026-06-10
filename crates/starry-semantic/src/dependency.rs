use std::collections::{HashMap, HashSet};

/// 依赖图：用于确定类型定义的求值顺序
///
/// 在多遍语义分析中，名称解析阶段会注册占位符符号及其依赖关系。
/// 依赖分析阶段使用此图进行拓扑排序，确定类型解析的顺序，
/// 并检测循环依赖。
///
/// 对应编译理论中依赖分析的标准做法：
/// 1. 收集所有符号及其依赖关系
/// 2. 构建有向图
/// 3. 执行拓扑排序（Kahn 算法）
/// 4. 若排序失败，则存在环（循环依赖）
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    /// 邻接表：名称 -> 依赖的名称列表
    edges: HashMap<String, Vec<String>>,
    /// 所有已知的节点（包括没有依赖的）
    nodes: HashSet<String>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            edges: HashMap::new(),
            nodes: HashSet::new(),
        }
    }

    /// 从依赖关系列表构建依赖图
    pub fn from_dependencies(deps: &[(String, Vec<String>)]) -> Self {
        let mut graph = Self::new();
        for (name, dependencies) in deps {
            graph.nodes.insert(name.clone());
            for dep in dependencies {
                graph.add_dependency(name, dep);
            }
        }
        graph
    }

    /// 添加一条依赖边：from 依赖 to
    pub fn add_dependency(&mut self, from: &str, to: &str) {
        self.nodes.insert(from.to_string());
        self.nodes.insert(to.to_string());
        self.edges
            .entry(from.to_string())
            .or_default()
            .push(to.to_string());
    }

    /// 拓扑排序
    ///
    /// 使用 Kahn 算法（BFS 风格），返回按依赖顺序排列的名称列表。
    /// 若检测到环，返回 Err 包含环路径。
    pub fn topological_sort(&self) -> Result<Vec<String>, Vec<String>> {
        let mut in_degree: HashMap<&String, usize> = HashMap::new();
        for node in &self.nodes {
            in_degree.insert(node, 0);
        }
        for (_, deps) in &self.edges {
            for dep in deps {
                if let Some(d) = in_degree.get_mut(dep) {
                    *d += 1;
                }
            }
        }

        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(node, _)| (*node).clone())
            .collect();

        let mut result = Vec::new();
        while let Some(node) = queue.pop() {
            result.push(node.clone());
            if let Some(deps) = self.edges.get(&node) {
                for dep in deps {
                    if let Some(d) = in_degree.get_mut(dep) {
                        *d -= 1;
                        if *d == 0 {
                            queue.push(dep.clone());
                        }
                    }
                }
            }
        }

        if result.len() == self.nodes.len() {
            Ok(result)
        } else {
            let cycle = self.detect_cycle();
            Err(cycle)
        }
    }

    /// 检测环（DFS 风格）
    fn detect_cycle(&self) -> Vec<String> {
        let mut visited = HashSet::new();
        let mut stack = Vec::new();
        let mut on_stack = HashSet::new();

        for node in &self.nodes {
            if !visited.contains(node) {
                if let Some(cycle) = self.dfs_cycle(node, &mut visited, &mut stack, &mut on_stack)
                {
                    return cycle;
                }
            }
        }

        Vec::new()
    }

    fn dfs_cycle<'a>(
        &self,
        node: &'a String,
        visited: &mut HashSet<String>,
        stack: &mut Vec<String>,
        on_stack: &mut HashSet<String>,
    ) -> Option<Vec<String>> {
        visited.insert(node.clone());
        stack.push(node.clone());
        on_stack.insert(node.clone());

        if let Some(deps) = self.edges.get(node) {
            for dep in deps {
                if on_stack.contains(dep) {
                    let cycle_start = stack.iter().position(|n| n == dep).unwrap();
                    let mut cycle = stack[cycle_start..].to_vec();
                    cycle.push(dep.clone());
                    return Some(cycle);
                }
                if !visited.contains(dep) {
                    if let Some(cycle) = self.dfs_cycle(dep, visited, stack, on_stack) {
                        return Some(cycle);
                    }
                }
            }
        }

        stack.pop();
        on_stack.remove(node);
        None
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|v| v.len()).sum()
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}
