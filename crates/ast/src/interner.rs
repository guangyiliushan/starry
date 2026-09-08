//! 标识符 interner：字符串 → Symbol(u32) 的唯一映射
//!
//! 每 parse session 独立实例（禁全局 static——并行测试各 session
//! 正是快照编号确定性的漂移防线）。泄漏指针即所有权，永不回收但
//! 单进程合法（rustc 同款，safe 代码）。

use std::collections::HashMap;

/// interned 标识符（插入序编号）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Symbol(pub u32);

/// 标识符 interner
///
/// `Vec<&'static str>` 持有泄漏指针即所有权；查重表以 `&'static str`
/// 为键（blanket `Borrow<str>` 免适配）。
#[derive(Debug, Default)]
pub struct Interner {
    strings: Vec<&'static str>,
    map: HashMap<&'static str, Symbol>,
}

impl Interner {
    pub fn new() -> Self {
        Self::default()
    }

    /// intern 一个字符串，返回 Symbol（幂等：重复返回同一 Symbol）
    pub fn intern(&mut self, s: &str) -> Symbol {
        if let Some(&sym) = self.map.get(s) {
            return sym;
        }
        let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
        let sym = Symbol(self.strings.len() as u32);
        self.strings.push(leaked);
        self.map.insert(leaked, sym);
        sym
    }

    /// resolve 一个 Symbol 为其字符串表示
    pub fn resolve(&self, sym: Symbol) -> &'static str {
        self.strings[sym.0 as usize]
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intern_idempotent() {
        let mut interner = Interner::new();
        let s1 = interner.intern("hello");
        let s2 = interner.intern("hello");
        assert_eq!(s1, s2);
        assert_eq!(interner.len(), 1);
        assert_eq!(interner.resolve(s1), "hello");
    }

    #[test]
    fn test_intern_distinct() {
        let mut interner = Interner::new();
        let a = interner.intern("a");
        let b = interner.intern("b");
        assert_ne!(a, b);
        assert_eq!(interner.resolve(a), "a");
        assert_eq!(interner.resolve(b), "b");
    }
}
