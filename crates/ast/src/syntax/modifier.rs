//! 共享辅件：修饰符列表（保序，带各自 span）

use super::node::Span;
use crate::token::ModifierWord;

/// 修饰符列表（保序，双指位诊断需要）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModifierList {
    pub modifiers: Vec<Modifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modifier {
    pub word: ModifierWord,
    pub span: Span,
}

impl ModifierList {
    pub fn new() -> Self {
        Self { modifiers: Vec::new() }
    }

    pub fn push(&mut self, word: ModifierWord, span: Span) {
        self.modifiers.push(Modifier { word, span });
    }

    pub fn has(&self, word: ModifierWord) -> bool {
        self.modifiers.iter().any(|m| m.word == word)
    }

    pub fn is_empty(&self) -> bool {
        self.modifiers.is_empty()
    }
}
