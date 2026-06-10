use crate::lr::item::{LR0Item, LR1Item};
use std::collections::{HashMap, HashSet};
use std::fmt;

/// 核心项集（忽略向前看符号）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreSet {
    items: HashSet<LR0Item>,
}

impl CoreSet {
    pub fn new() -> Self {
        CoreSet {
            items: HashSet::new(),
        }
    }

    pub fn from_items(items: impl IntoIterator<Item = LR0Item>) -> Self {
        CoreSet {
            items: items.into_iter().collect(),
        }
    }

    pub fn from_lr1_items(items: impl IntoIterator<Item = LR1Item>) -> Self {
        CoreSet {
            items: items.into_iter().map(|item| item.core()).collect(),
        }
    }

    pub fn insert(&mut self, item: LR0Item) -> bool {
        self.items.insert(item)
    }

    pub fn contains(&self, item: &LR0Item) -> bool {
        self.items.contains(item)
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &LR0Item> {
        self.items.iter()
    }

    pub fn items(&self) -> &HashSet<LR0Item> {
        &self.items
    }
}

impl Default for CoreSet {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CoreSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Core({{ ")?;
        let mut items: Vec<_> = self.items.iter().collect();
        items.sort();
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}", item)?;
        }
        write!(f, " }})")
    }
}

/// 同心集合并器
///
/// 用于 LALR(1) 解析器，合并具有相同核心的 LR(1) 项集
pub struct CoreSetMerger {
    core_to_state: HashMap<Vec<LR0Item>, usize>,
    states: Vec<HashSet<LR1Item>>,
}

impl CoreSetMerger {
    pub fn new() -> Self {
        CoreSetMerger {
            core_to_state: HashMap::new(),
            states: Vec::new(),
        }
    }

    pub fn add_or_merge(&mut self, items: HashSet<LR1Item>) -> usize {
        let mut core_vec: Vec<LR0Item> =
            items.iter().map(|item| item.core()).collect::<HashSet<_>>().into_iter().collect();
        core_vec.sort();

        if let Some(&state_id) = self.core_to_state.get(&core_vec) {
            self.merge_lookaheads(state_id, &items);
            state_id
        } else {
            let state_id = self.states.len();
            self.core_to_state.insert(core_vec, state_id);
            self.states.push(items);
            state_id
        }
    }

    fn merge_lookaheads(&mut self, state_id: usize, new_items: &HashSet<LR1Item>) {
        let existing = &mut self.states[state_id];

        for new_item in new_items {
            let mut found = false;
            for existing_item in existing.iter() {
                if existing_item.core() == new_item.core() {
                    found = true;
                    break;
                }
            }

            if !found {
                existing.insert(new_item.clone());
            } else {
                let core = new_item.core();
                let mut to_insert = Vec::new();
                for existing_item in existing.iter() {
                    if existing_item.core() == core && existing_item.lookahead != new_item.lookahead {
                        to_insert.push(new_item.clone());
                        break;
                    }
                }
                for item in to_insert {
                    existing.insert(item);
                }
            }
        }
    }

    pub fn get_state(&self, state_id: usize) -> Option<&HashSet<LR1Item>> {
        self.states.get(state_id)
    }

    pub fn state_count(&self) -> usize {
        self.states.len()
    }

    pub fn states(&self) -> &[HashSet<LR1Item>] {
        &self.states
    }

    pub fn into_states(self) -> Vec<HashSet<LR1Item>> {
        self.states
    }
}

impl Default for CoreSetMerger {
    fn default() -> Self {
        Self::new()
    }
}

/// 同心集检测器
pub struct CoreSetDetector;

impl CoreSetDetector {
    pub fn are_core_equivalent(set1: &HashSet<LR1Item>, set2: &HashSet<LR1Item>) -> bool {
        let core1: HashSet<LR0Item> = set1.iter().map(|item| item.core()).collect();
        let core2: HashSet<LR0Item> = set2.iter().map(|item| item.core()).collect();
        core1 == core2
    }

    pub fn get_core(items: &HashSet<LR1Item>) -> CoreSet {
        CoreSet::from_lr1_items(items.iter().cloned())
    }

    pub fn group_by_core(item_sets: &[HashSet<LR1Item>]) -> HashMap<Vec<LR0Item>, Vec<usize>> {
        let mut groups: HashMap<Vec<LR0Item>, Vec<usize>> = HashMap::new();

        for (idx, item_set) in item_sets.iter().enumerate() {
            let mut core_vec: Vec<LR0Item> = item_set.iter().map(|item| item.core()).collect();
            core_vec.sort();
            groups.entry(core_vec).or_default().push(idx);
        }

        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::TerminalId;

    #[test]
    fn test_core_set_creation() {
        let mut core = CoreSet::new();
        core.insert(LR0Item::new(0, 0));
        core.insert(LR0Item::new(1, 1));

        assert_eq!(core.len(), 2);
    }

    #[test]
    fn test_core_set_from_lr1() {
        let mut lr1_items = HashSet::new();
        lr1_items.insert(LR1Item::new(0, 0, TerminalId(0)));
        lr1_items.insert(LR1Item::new(0, 0, TerminalId(1)));

        let core = CoreSet::from_lr1_items(lr1_items);

        assert_eq!(core.len(), 1);
    }

    #[test]
    fn test_core_set_merger() {
        let mut merger = CoreSetMerger::new();

        let mut set1 = HashSet::new();
        set1.insert(LR1Item::new(0, 0, TerminalId(0)));

        let id1 = merger.add_or_merge(set1);
        assert_eq!(id1, 0);

        let mut set2 = HashSet::new();
        set2.insert(LR1Item::new(0, 0, TerminalId(1)));

        let id2 = merger.add_or_merge(set2);
        assert_eq!(id2, 0);

        assert_eq!(merger.state_count(), 1);
        let merged = merger.get_state(0).unwrap();
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_core_equivalence() {
        let mut set1 = HashSet::new();
        set1.insert(LR1Item::new(0, 0, TerminalId(0)));
        set1.insert(LR1Item::new(1, 1, TerminalId(0)));

        let mut set2 = HashSet::new();
        set2.insert(LR1Item::new(0, 0, TerminalId(1)));
        set2.insert(LR1Item::new(1, 1, TerminalId(2)));

        assert!(CoreSetDetector::are_core_equivalent(&set1, &set2));
    }
}
