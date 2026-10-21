//! 标识符 interner：字符串 → Symbol(u32) 的唯一映射
//!
//! 每 parse session 独立实例（禁全局 static——并行测试各 session
//! 正是快照编号确定性的漂移防线）。`Rc<str>` 由 interner 持有；
//! `Symbol` 只有在所属 `ParseOutput.interner` 存活期间可 resolve。

use std::collections::HashMap;
use std::rc::Rc;

/// interned 标识符（插入序编号）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Symbol(pub u32);

/// 标识符 interner
///
/// 字符串由 `Vec<Rc<str>>` 拥有；查重表共享同一 allocation。
#[derive(Debug, Default)]
pub struct Interner {
    strings: Vec<Rc<str>>,
    map: HashMap<Rc<str>, Symbol>,
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
        let owned: Rc<str> = Rc::from(s);
        let sym = Symbol(self.strings.len() as u32);
        self.strings.push(Rc::clone(&owned));
        self.map.insert(owned, sym);
        sym
    }

    /// resolve 一个 Symbol 为其字符串表示
    pub fn resolve(&self, sym: Symbol) -> &str {
        &self.strings[sym.0 as usize]
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
