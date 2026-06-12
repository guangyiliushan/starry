//! 迭代遍历器模块
//!
//! 该模块提供 AST 和 HIR 的迭代遍历能力，避免递归导致的栈溢出。
//!
//! # 设计特点
//!
//! - **迭代遍历**：使用显式栈，避免递归导致的栈溢出
//! - **支持 DFS 和 BFS**：提供两种遍历方式
//! - **泛型设计**：可以自定义访问操作
//! - **错误传播**：支持在遍历过程中返回错误
//!
//! # 核心类型
//!
//! - [`Visitor`] - 通用的迭代访问器
//! - [`visit`] - 便捷的深度优先遍历函数
//!
//! # 示例
//!
//! ```
//! # use lex::regex::ast::Ast;
//! # use lex::regex::ast::literal;
//! # use lex::regex::visitor;
//!
//! let ast = Ast::sequence(vec![literal('a'), literal('b'), literal('c')]);
//!
//! // 统计 AST 中的字面量数量
//! let mut count = 0;
//! visitor::visit(&ast, |node| {
//!     if matches!(node, Ast::Literal(_)) {
//!         count += 1;
//!     }
//!     Ok(())
//! }).unwrap();
//!
//! assert_eq!(count, 3);
//! ```

use std::collections::VecDeque;
use crate::regex::ast::Ast;
use crate::regex::parse::ParseError;

// ==================== 迭代访问器 ====================

/// 通用的迭代访问器
///
/// 使用显式栈而非递归，避免深层嵌套导致的栈溢出。
///
/// # 类型参数
///
/// - `F` - 访问函数类型，接受 `&Ast` 并返回 `Result<(), ParseError>`
///
/// # 示例
///
/// ```
/// # use lex::regex::ast::Ast;
/// # use lex::regex::visitor::Visitor;
///
/// let ast = Ast::sequence(vec![Ast::Literal('a'), Ast::Literal('b')]);
///
/// let mut nodes = Vec::new();
/// let mut visitor = Visitor::new(&ast, |node| {
///     nodes.push(format!("{:?}", node));
///     Ok(())
/// });
///
/// visitor.dfs().unwrap();
/// ```
pub struct Visitor<'a, F>
where
    F: FnMut(&Ast) -> Result<(), ParseError>,
{
    stack: Vec<&'a Ast>,
    visit: F,
}

impl<'a, F> Visitor<'a, F>
where
    F: FnMut(&Ast) -> Result<(), ParseError>,
{
    /// 创建新的访问器
    ///
    /// # 参数
    ///
    /// - `root` - 要遍历的 AST 根节点
    /// - `visit` - 访问函数
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::ast::Ast;
    /// # use lex::regex::visitor::Visitor;
    ///
    /// let ast = Ast::Literal('a');
    /// let visitor = Visitor::new(&ast, |node| {
    ///     println!("{:?}", node);
    ///     Ok(())
    /// });
    /// ```
    pub fn new(root: &'a Ast, visit: F) -> Self {
        Self {
            stack: vec![root],
            visit,
        }
    }

    /// 深度优先遍历（DFS）
    ///
    /// 使用栈实现深度优先遍历，避免递归。
    ///
    /// # 返回
    ///
    /// 如果所有访问都成功，返回 `Ok(())`
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::ast::Ast;
    /// # use lex::regex::visitor::Visitor;
    ///
    /// let ast = Ast::sequence(vec![Ast::Literal('a'), Ast::Literal('b')]);
    /// let mut visitor = Visitor::new(&ast, |node| {
    ///     println!("{:?}", node);
    ///     Ok(())
    /// });
    ///
    /// visitor.dfs().unwrap();
    /// ```
    pub fn dfs(&mut self) -> Result<(), ParseError> {
        while let Some(node) = self.stack.pop() {
            (self.visit)(node)?;
            
            // 反向压栈，保证 DFS 顺序
            for child in node.children().into_iter().rev() {
                self.stack.push(child);
            }
        }
        
        Ok(())
    }

    /// 广度优先遍历（BFS）
    ///
    /// 使用队列实现广度优先遍历。
    ///
    /// # 返回
    ///
    /// 如果所有访问都成功，返回 `Ok(())`
    ///
    /// # 示例
    ///
    /// ```
    /// # use lex::regex::ast::Ast;
    /// # use lex::regex::visitor::Visitor;
    ///
    /// let ast = Ast::sequence(vec![Ast::Literal('a'), Ast::Literal('b')]);
    /// let mut visitor = Visitor::new(&ast, |node| {
    ///     println!("{:?}", node);
    ///     Ok(())
    /// });
    ///
    /// visitor.bfs().unwrap();
    /// ```
    pub fn bfs(&mut self) -> Result<(), ParseError> {
        let mut queue: VecDeque<&'a Ast> = self.stack.drain(..).collect();
        
        while let Some(node) = queue.pop_front() {
            (self.visit)(node)?;
            
            // 添加子节点到队列末尾
            for child in node.children() {
                queue.push_back(child);
            }
        }
        
        Ok(())
    }
}

// ==================== 便捷函数 ====================

/// 深度优先遍历 AST（便捷函数）
///
/// # 参数
///
/// - `ast` - 要遍历的 AST
/// - `visit` - 访问函数
///
/// # 返回
///
/// 如果所有访问都成功，返回 `Ok(())`
///
/// # 示例
///
/// ```
/// # use lex::regex::ast::Ast;
/// # use lex::regex::visitor;
///
/// let ast = Ast::sequence(vec![Ast::Literal('a'), Ast::Literal('b')]);
///
/// visitor::visit(&ast, |node| {
///     println!("{:?}", node);
///     Ok(())
/// }).unwrap();
/// ```
pub fn visit<F>(ast: &Ast, visit: F) -> Result<(), ParseError>
where
    F: FnMut(&Ast) -> Result<(), ParseError>,
{
    Visitor::new(ast, visit).dfs()
}

/// 广度优先遍历 AST（便捷函数）
///
/// # 参数
///
/// - `ast` - 要遍历的 AST
/// - `visit` - 访问函数
///
/// # 返回
///
/// 如果所有访问都成功，返回 `Ok(())`
///
/// # 示例
///
/// ```
/// # use lex::regex::ast::Ast;
/// # use lex::regex::visitor;
///
/// let ast = Ast::sequence(vec![Ast::Literal('a'), Ast::Literal('b')]);
///
/// visitor::visit_bfs(&ast, |node| {
///     println!("{:?}", node);
///     Ok(())
/// }).unwrap();
/// ```
pub fn visit_bfs<F>(ast: &Ast, visit: F) -> Result<(), ParseError>
where
    F: FnMut(&Ast) -> Result<(), ParseError>,
{
    Visitor::new(ast, visit).bfs()
}

/// 收集 AST 中的所有节点
///
/// # 参数
///
/// - `ast` - 要遍历的 AST
///
/// # 返回
///
/// 包含所有 AST 节点的向量
///
/// # 示例
///
/// ```
/// # use lex::regex::ast::Ast;
/// # use lex::regex::visitor;
///
/// let ast = Ast::sequence(vec![Ast::Literal('a'), Ast::Literal('b')]);
/// let nodes = visitor::collect(&ast);
///
/// assert_eq!(nodes.len(), 3); // Sequence, Literal('a'), Literal('b')
/// ```
pub fn collect(ast: &Ast) -> Vec<Ast> {
    let mut nodes = Vec::new();
    visit(ast, |node| {
        nodes.push(node.clone());
        Ok(())
    }).unwrap();
    nodes
}

/// 统计 AST 中的节点数量
///
/// # 参数
///
/// - `ast` - 要遍历的 AST
///
/// # 返回
///
/// AST 节点的数量
///
/// # 示例
///
/// ```
/// # use lex::regex::ast::Ast;
/// # use lex::regex::visitor;
///
/// let ast = Ast::sequence(vec![Ast::Literal('a'), Ast::Literal('b')]);
/// let count = visitor::count(&ast);
///
/// assert_eq!(count, 3);
/// ```
pub fn count(ast: &Ast) -> usize {
    collect(ast).len()
}

/// 检查 AST 是否包含特定节点类型
///
/// # 参数
///
/// - `ast` - 要遍历的 AST
/// - `predicate` - 谓词函数
///
/// # 返回
///
/// 如果存在满足条件的节点，返回 true
///
/// # 示例
///
/// ```
/// # use lex::regex::ast::Ast;
/// # use lex::regex::visitor;
///
/// let ast = Ast::sequence(vec![Ast::Literal('a'), Ast::Literal('b')]);
/// let has_literal = visitor::any(&ast, |node| {
///     matches!(node, Ast::Literal(_))
/// });
///
/// assert!(has_literal);
/// ```
pub fn any<F>(ast: &Ast, predicate: F) -> bool
where
    F: Fn(&Ast) -> bool,
{
    let mut result = false;
    visit(ast, |node| {
        if predicate(node) {
            result = true;
        }
        Ok(())
    }).ok();
    result
}

/// 检查 AST 的所有节点是否都满足条件
///
/// # 参数
///
/// - `ast` - 要遍历的 AST
/// - `predicate` - 谓词函数
///
/// # 返回
///
/// 如果所有节点都满足条件，返回 true
///
/// # 示例
///
/// ```
/// # use lex::regex::ast::Ast;
/// # use lex::regex::visitor;
///
/// let ast = Ast::sequence(vec![Ast::Literal('a'), Ast::Literal('b')]);
/// let all_literal = visitor::all(&ast, |node| {
///     !matches!(node, Ast::Sequence(_))
/// });
///
/// assert!(!all_literal); // Sequence 节点不满足条件
/// ```
pub fn all<F>(ast: &Ast, predicate: F) -> bool
where
    F: Fn(&Ast) -> bool,
{
    let mut result = true;
    visit(ast, |node| {
        if !predicate(node) {
            result = false;
        }
        Ok(())
    }).ok();
    result
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::regex::ast::literal;

    #[test]
    fn test_visitor_dfs() {
        let ast = Ast::sequence(vec![
            literal('a'),
            literal('b'),
        ]);

        let mut nodes = Vec::new();
        let mut visitor = Visitor::new(&ast, |node| {
            nodes.push(format!("{:?}", node));
            Ok(())
        });

        visitor.dfs().unwrap();
        
        // DFS 顺序：Sequence, Literal('b'), Literal('a')
        assert!(nodes.iter().any(|s| s.contains("Sequence")));
        assert!(nodes.iter().any(|s| s.contains("Literal('a')")));
        assert!(nodes.iter().any(|s| s.contains("Literal('b')")));
    }

    #[test]
    fn test_visitor_bfs() {
        let ast = Ast::sequence(vec![
            literal('a'),
            literal('b'),
        ]);

        let mut nodes = Vec::new();
        let mut visitor = Visitor::new(&ast, |node| {
            nodes.push(format!("{:?}", node));
            Ok(())
        });

        visitor.bfs().unwrap();
        
        // BFS 顺序：Sequence, Literal('a'), Literal('b')
        assert!(nodes.iter().any(|s| s.contains("Sequence")));
        assert!(nodes.iter().any(|s| s.contains("Literal('a')")));
        assert!(nodes.iter().any(|s| s.contains("Literal('b')")));
    }

    #[test]
    fn test_visit() {
        let ast = Ast::sequence(vec![literal('a'), literal('b')]);
        
        let mut count = 0;
        visit(&ast, |_node| {
            count += 1;
            Ok(())
        }).unwrap();
        
        assert_eq!(count, 3);
    }

    #[test]
    fn test_collect() {
        let ast = Ast::sequence(vec![literal('a'), literal('b')]);
        let nodes = collect(&ast);
        
        assert_eq!(nodes.len(), 3);
    }

    #[test]
    fn test_count() {
        let ast = Ast::sequence(vec![literal('a'), literal('b')]);
        let count = count(&ast);
        
        assert_eq!(count, 3);
    }

    #[test]
    fn test_any() {
        let ast = Ast::sequence(vec![literal('a'), literal('b')]);
        let has_literal = any(&ast, |node| {
            matches!(node, Ast::Literal(_))
        });
        
        assert!(has_literal);
    }

    #[test]
    fn test_any_false() {
        let ast = Ast::sequence(vec![literal('a'), literal('b')]);
        let has_choice = any(&ast, |node| {
            matches!(node, Ast::Choice(_))
        });
        
        assert!(!has_choice);
    }

    #[test]
    fn test_all() {
        let ast = Ast::sequence(vec![literal('a'), literal('b')]);
        let not_all_literal = all(&ast, |node| {
            matches!(node, Ast::Literal(_))
        });
        
        assert!(!not_all_literal); // Sequence 节点不满足
    }

    #[test]
    fn test_all_true() {
        let ast = Ast::sequence(vec![literal('a'), literal('b')]);
        let all_not_empty = all(&ast, |node| {
            !matches!(node, Ast::Empty)
        });
        
        assert!(all_not_empty);
    }

    #[test]
    fn test_deep_nesting() {
        // 测试深层嵌套不会导致栈溢出
        let mut ast: Ast = Ast::literal('a');
        for _ in 0..1000 {
            ast = Ast::sequence(vec![ast, Ast::literal('b')]);
        }
        
        let count = count(&ast);
        assert_eq!(count, 1002); // Sequence + Literal('a') + 1000 * Literal('b')
    }
}