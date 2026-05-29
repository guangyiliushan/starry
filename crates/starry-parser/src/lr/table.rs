use crate::analysis::FollowSet;
use crate::cfg::{ContextFreeGrammar, Symbol, TerminalId};
use crate::lr::action::{Action, ActionTable, ProductionId};
use crate::lr::augmented::AugmentedGrammar;
use crate::lr::closure::LR0Closure;
use crate::lr::conflict::{Conflict, ConflictReport};
use crate::lr::core_set::CoreSetMerger;
use crate::lr::follow::LRFollowCalculator;
use crate::lr::goto::{GotoTable, ItemSetId, LR0Goto};
use crate::lr::item::LR0Item;
use crate::lr::lookahead::LookaheadCalculator;
use crate::lr::states::{LR0ItemSetCollection, LR1ItemSetCollection};
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrammarType {
    LR0,
    SLR1,
    LR1,
    LALR1,
    NotLR,
    Unknown,
}

impl fmt::Display for GrammarType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GrammarType::LR0 => write!(f, "LR(0)"),
            GrammarType::SLR1 => write!(f, "SLR(1)"),
            GrammarType::LR1 => write!(f, "LR(1)"),
            GrammarType::LALR1 => write!(f, "LALR(1)"),
            GrammarType::NotLR => write!(f, "Not LR"),
            GrammarType::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum LRTable {
    LR0(LR0Table),
    SLR1(SLRTable),
    LR1(LR1Table),
    LALR1(LALR1Table),
}

impl LRTable {
    pub fn action(&self) -> &ActionTable {
        match self {
            LRTable::LR0(t) => &t.action,
            LRTable::SLR1(t) => &t.action,
            LRTable::LR1(t) => &t.action,
            LRTable::LALR1(t) => &t.action,
        }
    }

    pub fn goto(&self) -> &GotoTable {
        match self {
            LRTable::LR0(t) => &t.goto,
            LRTable::SLR1(t) => &t.goto,
            LRTable::LR1(t) => &t.goto,
            LRTable::LALR1(t) => &t.goto,
        }
    }

    pub fn has_conflicts(&self) -> bool {
        match self {
            LRTable::LR0(t) => t.has_conflicts(),
            LRTable::SLR1(t) => t.has_conflicts(),
            LRTable::LR1(t) => t.has_conflicts(),
            LRTable::LALR1(t) => t.has_conflicts(),
        }
    }

    pub fn state_count(&self) -> usize {
        match self {
            LRTable::LR0(t) => t.state_count,
            LRTable::SLR1(t) => t.state_count,
            LRTable::LR1(t) => t.state_count,
            LRTable::LALR1(t) => t.state_count,
        }
    }

    pub fn grammar_type(&self) -> GrammarType {
        match self {
            LRTable::LR0(_) => GrammarType::LR0,
            LRTable::SLR1(_) => GrammarType::SLR1,
            LRTable::LR1(_) => GrammarType::LR1,
            LRTable::LALR1(_) => GrammarType::LALR1,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LR0Table {
    pub action: ActionTable,
    pub goto: GotoTable,
    pub state_count: usize,
    pub terminal_count: usize,
    pub non_terminal_count: usize,
    pub conflicts: Vec<Conflict>,
}

impl LR0Table {
    pub fn new(state_count: usize, terminal_count: usize, non_terminal_count: usize) -> Self {
        LR0Table {
            action: ActionTable::new(state_count, terminal_count),
            goto: GotoTable::new(state_count, non_terminal_count),
            state_count,
            terminal_count,
            non_terminal_count,
            conflicts: Vec::new(),
        }
    }

    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    pub fn conflict_report(&self) -> ConflictReport {
        ConflictReport::new(self.conflicts.clone())
    }

    pub fn grammar_type(&self) -> GrammarType {
        if self.conflicts.is_empty() {
            GrammarType::LR0
        } else {
            GrammarType::NotLR
        }
    }

    pub fn is_lr0(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn print(&self, cfg: &ContextFreeGrammar) {
        println!("LR(0) Parsing Table:");
        println!();
        
        let terminal_names: Vec<String> = (0..cfg.terminals.len())
            .map(|i| cfg.get_terminal_name(crate::cfg::TerminalId(i)).to_string())
            .collect();
        let non_terminal_names: Vec<String> = (0..cfg.non_terminals.len())
            .map(|i| cfg.get_non_terminal_name(crate::cfg::NonTerminalId(i)).to_string())
            .collect();
        
        self.action.print(&terminal_names);
        println!();
        self.goto.print(&non_terminal_names);
        
        if self.has_conflicts() {
            println!();
            println!("Conflicts:");
            for conflict in &self.conflicts {
                println!("  {}", conflict);
            }
        }
    }
}

impl fmt::Display for LR0Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "LR(0) Parsing Table:")?;
        writeln!(f, "{}", self.action)?;
        writeln!(f, "{}", self.goto)?;
        if self.has_conflicts() {
            writeln!(f, "\nConflicts:")?;
            for conflict in &self.conflicts {
                writeln!(f, "  {}", conflict)?;
            }
        }
        Ok(())
    }
}

pub struct LR0TableBuilder;

impl LR0TableBuilder {
    pub fn build(augmented: &AugmentedGrammar) -> LR0Table {
        let collection = LR0ItemSetCollection::build_from_augmented(augmented);
        Self::build_from_collection(augmented, &collection)
    }

    pub fn build_from_collection(
        augmented: &AugmentedGrammar,
        collection: &LR0ItemSetCollection,
    ) -> LR0Table {
        let cfg = augmented.grammar();
        let state_count = collection.len();
        let terminal_count = cfg.terminals.len();
        let non_terminal_count = cfg.non_terminals.len();
        let end_marker = TerminalId(terminal_count);

        let mut table = LR0Table::new(state_count, terminal_count, non_terminal_count);

        for (state_idx, item_set) in collection.states().iter().enumerate() {
            let state_id = ItemSetId(state_idx);

            for item in item_set.items() {
                if let Some(production) = cfg.productions.get(item.production_index) {
                    if item.is_complete(production) {
                        if augmented.is_accepting_production(item.production_index) {
                            table.action.set_accept(state_id, end_marker);
                        } else {
                            for term_idx in 0..=terminal_count {
                                let terminal = TerminalId(term_idx);
                                table.action.set_reduce(
                                    state_id,
                                    terminal,
                                    ProductionId(item.production_index),
                                );
                            }
                        }
                    } else if let Some(Symbol::Terminal(term_id)) = item.next_symbol(production) {
                        if let Some(next_state) =
                            collection.get_transition(state_idx, &Symbol::Terminal(*term_id))
                        {
                            table.action.set_shift(state_id, *term_id, ItemSetId(next_state));
                        }
                    } else if let Some(Symbol::NonTerminal(nt_id)) = item.next_symbol(production) {
                        if let Some(next_state) =
                            collection.get_transition(state_idx, &Symbol::NonTerminal(*nt_id))
                        {
                            table.goto.set(state_id, *nt_id, ItemSetId(next_state));
                        }
                    }
                }
            }
        }

        table.conflicts = Self::detect_conflicts(&table, collection, augmented);
        table
    }

    fn detect_conflicts(
        table: &LR0Table,
        collection: &LR0ItemSetCollection,
        augmented: &AugmentedGrammar,
    ) -> Vec<Conflict> {
        let mut conflicts = Vec::new();
        let cfg = augmented.grammar();

        for (state_id, item_set) in collection.states().iter().enumerate() {
            for term_idx in 0..=cfg.terminals.len() {
                let terminal = TerminalId(term_idx);
                let action = table.action.get(ItemSetId(state_id), terminal);

                if let Action::Shift(shift_state) = action {
                    for item in item_set.items() {
                        if let Some(production) = cfg.productions.get(item.production_index) {
                            if item.is_complete(production) {
                                conflicts.push(Conflict::shift_reduce(
                                    ItemSetId(state_id),
                                    terminal,
                                    *shift_state,
                                    ProductionId(item.production_index),
                                ));
                            }
                        }
                    }
                }
            }
        }

        conflicts
    }
}

#[derive(Debug, Clone)]
pub struct SLRTable {
    pub action: ActionTable,
    pub goto: GotoTable,
    pub state_count: usize,
    pub terminal_count: usize,
    pub non_terminal_count: usize,
    pub conflicts: Vec<Conflict>,
    pub resolved_conflicts: Vec<Conflict>,
}

impl SLRTable {
    pub fn new(state_count: usize, terminal_count: usize, non_terminal_count: usize) -> Self {
        SLRTable {
            action: ActionTable::new(state_count, terminal_count),
            goto: GotoTable::new(state_count, non_terminal_count),
            state_count,
            terminal_count,
            non_terminal_count,
            conflicts: Vec::new(),
            resolved_conflicts: Vec::new(),
        }
    }

    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    pub fn conflict_report(&self) -> ConflictReport {
        ConflictReport::new(self.conflicts.clone())
    }

    pub fn grammar_type(&self) -> GrammarType {
        if self.conflicts.is_empty() {
            if self.resolved_conflicts.is_empty() {
                GrammarType::LR0
            } else {
                GrammarType::SLR1
            }
        } else {
            GrammarType::NotLR
        }
    }

    pub fn is_slr1(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn is_lr0(&self) -> bool {
        self.conflicts.is_empty() && self.resolved_conflicts.is_empty()
    }

    pub fn print(&self, cfg: &ContextFreeGrammar) {
        println!("SLR(1) Parsing Table:");
        println!();
        
        let terminal_names: Vec<String> = (0..cfg.terminals.len())
            .map(|i| cfg.get_terminal_name(crate::cfg::TerminalId(i)).to_string())
            .collect();
        let non_terminal_names: Vec<String> = (0..cfg.non_terminals.len())
            .map(|i| cfg.get_non_terminal_name(crate::cfg::NonTerminalId(i)).to_string())
            .collect();
        
        self.action.print(&terminal_names);
        println!();
        self.goto.print(&non_terminal_names);
        
        if !self.resolved_conflicts.is_empty() {
            println!();
            println!("Resolved conflicts by FOLLOW sets:");
            for conflict in &self.resolved_conflicts {
                println!("  {}", conflict);
            }
        }
        if self.has_conflicts() {
            println!();
            println!("Unresolved conflicts:");
            for conflict in &self.conflicts {
                println!("  {}", conflict);
            }
        }
    }
}

impl fmt::Display for SLRTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "SLR(1) Parsing Table:")?;
        writeln!(f, "{}", self.action)?;
        writeln!(f, "{}", self.goto)?;
        if !self.resolved_conflicts.is_empty() {
            writeln!(f, "\nResolved conflicts by FOLLOW sets:")?;
            for conflict in &self.resolved_conflicts {
                writeln!(f, "  {}", conflict)?;
            }
        }
        if self.has_conflicts() {
            writeln!(f, "\nUnresolved conflicts:")?;
            for conflict in &self.conflicts {
                writeln!(f, "  {}", conflict)?;
            }
        }
        Ok(())
    }
}

pub struct SLRTableBuilder;

impl SLRTableBuilder {
    pub fn build(augmented: &AugmentedGrammar) -> SLRTable {
        let cfg = augmented.grammar();
        let follow_sets = LRFollowCalculator::compute(cfg);
        let end_marker = TerminalId(cfg.terminals.len());

        let states = Self::build_states(augmented);
        let state_count = states.len();
        let terminal_count = cfg.terminals.len();
        let non_terminal_count = cfg.non_terminals.len();

        let mut table = SLRTable::new(state_count, terminal_count, non_terminal_count);

        let state_map = Self::build_state_map(&states);

        for (state_id, item_set) in states.iter().enumerate() {
            Self::fill_table_for_state(
                &mut table,
                ItemSetId(state_id),
                item_set,
                augmented,
                &follow_sets,
                end_marker,
                &states,
                &state_map,
            );
        }

        let (conflicts, resolved) = Self::detect_and_resolve_conflicts(&table, &states, augmented, &follow_sets);
        table.conflicts = conflicts;
        table.resolved_conflicts = resolved;

        table
    }

    fn build_states(augmented: &AugmentedGrammar) -> Vec<HashSet<LR0Item>> {
        let cfg = augmented.grammar();
        let mut states: Vec<HashSet<LR0Item>> = Vec::new();
        let mut state_map: HashMap<Vec<LR0Item>, usize> = HashMap::new();
        let mut worklist: Vec<usize> = Vec::new();

        let initial_item = LR0Item::from_production(0);
        let mut initial_items = HashSet::new();
        initial_items.insert(initial_item);
        let initial_closure = LR0Closure::compute(&initial_items, cfg);

        let mut sorted_initial: Vec<_> = initial_closure.iter().copied().collect();
        sorted_initial.sort();
        state_map.insert(sorted_initial.clone(), 0);
        states.push(initial_closure);
        worklist.push(0);

        while let Some(state_idx) = worklist.pop() {
            let transitions: Vec<Symbol> = {
                let current_state = &states[state_idx];
                LR0Goto::get_transitions(current_state, cfg)
            };

            for symbol in transitions {
                let new_items_opt = {
                    let current_state = &states[state_idx];
                    LR0Goto::compute(current_state, cfg, &symbol)
                };

                if let Some(new_items) = new_items_opt {
                    let mut sorted_items: Vec<_> = new_items.iter().copied().collect();
                    sorted_items.sort();

                    if state_map.contains_key(&sorted_items) {
                        continue;
                    }

                    let new_idx = states.len();
                    state_map.insert(sorted_items, new_idx);
                    states.push(new_items);
                    worklist.push(new_idx);
                }
            }
        }

        states
    }

    fn build_state_map(states: &[HashSet<LR0Item>]) -> HashMap<Vec<LR0Item>, usize> {
        let mut map = HashMap::new();
        for (idx, state) in states.iter().enumerate() {
            let mut sorted: Vec<_> = state.iter().copied().collect();
            sorted.sort();
            map.insert(sorted, idx);
        }
        map
    }

    #[allow(clippy::too_many_arguments)]
    fn fill_table_for_state(
        table: &mut SLRTable,
        state_id: ItemSetId,
        item_set: &HashSet<LR0Item>,
        augmented: &AugmentedGrammar,
        follow_sets: &FollowSet,
        end_marker: TerminalId,
        _states: &[HashSet<LR0Item>],
        state_map: &HashMap<Vec<LR0Item>, usize>,
    ) {
        let cfg = augmented.grammar();

        for item in item_set {
            if let Some(production) = cfg.productions.get(item.production_index) {
                if item.is_complete(production) {
                    if augmented.is_accepting_production(item.production_index) {
                        table.action.set_accept(state_id, end_marker);
                    } else {
                        let lhs = production.lhs;
                        for terminal in follow_sets.get(lhs) {
                            table.action.set_reduce(
                                state_id,
                                *terminal,
                                ProductionId(item.production_index),
                            );
                        }
                    }
                } else if let Some(Symbol::Terminal(term_id)) = item.next_symbol(production) {
                    if let Some(new_items) = LR0Goto::compute(item_set, cfg, &Symbol::Terminal(*term_id)) {
                        if let Some(new_state_id) = Self::find_state_index(&new_items, state_map) {
                            table.action.set_shift(state_id, *term_id, ItemSetId(new_state_id));
                        }
                    }
                }
            }
        }

        for item in item_set {
            if let Some(production) = cfg.productions.get(item.production_index) {
                if let Some(Symbol::NonTerminal(nt_id)) = item.next_symbol(production) {
                    if let Some(new_items) = LR0Goto::compute(item_set, cfg, &Symbol::NonTerminal(*nt_id)) {
                        if let Some(new_state_id) = Self::find_state_index(&new_items, state_map) {
                            table.goto.set(state_id, *nt_id, ItemSetId(new_state_id));
                        }
                    }
                }
            }
        }
    }

    fn find_state_index(
        items: &HashSet<LR0Item>,
        state_map: &HashMap<Vec<LR0Item>, usize>,
    ) -> Option<usize> {
        let mut sorted: Vec<_> = items.iter().copied().collect();
        sorted.sort();
        state_map.get(&sorted).copied()
    }

    fn detect_and_resolve_conflicts(
        _table: &SLRTable,
        states: &[HashSet<LR0Item>],
        augmented: &AugmentedGrammar,
        follow_sets: &FollowSet,
    ) -> (Vec<Conflict>, Vec<Conflict>) {
        let mut unresolved_conflicts = Vec::new();
        let resolved_conflicts = Vec::new();
        let cfg = augmented.grammar();

        for (state_id, item_set) in states.iter().enumerate() {
            let mut shift_items: Vec<(TerminalId, ItemSetId)> = Vec::new();
            let mut reduce_items: Vec<(TerminalId, ProductionId)> = Vec::new();

            for item in item_set {
                if let Some(production) = cfg.productions.get(item.production_index) {
                    if item.is_complete(production) {
                        if !augmented.is_accepting_production(item.production_index) {
                            let lhs = production.lhs;
                            for terminal in follow_sets.get(lhs) {
                                reduce_items.push((*terminal, ProductionId(item.production_index)));
                            }
                        }
                    } else if let Some(Symbol::Terminal(term_id)) = item.next_symbol(production) {
                        if let Some(new_items) = LR0Goto::compute(item_set, cfg, &Symbol::Terminal(*term_id)) {
                            let state_map = Self::build_state_map(states);
                            if let Some(new_state_id) = Self::find_state_index(&new_items, &state_map) {
                                shift_items.push((*term_id, ItemSetId(new_state_id)));
                            }
                        }
                    }
                }
            }

            for (term, shift_state) in &shift_items {
                for (reduce_term, prod_id) in &reduce_items {
                    if term == reduce_term {
                        let conflict = Conflict::shift_reduce(
                            ItemSetId(state_id),
                            *term,
                            *shift_state,
                            *prod_id,
                        );
                        unresolved_conflicts.push(conflict);
                    }
                }
            }

            for i in 0..reduce_items.len() {
                for j in (i + 1)..reduce_items.len() {
                    if reduce_items[i].0 == reduce_items[j].0 {
                        let conflict = Conflict::reduce_reduce(
                            ItemSetId(state_id),
                            reduce_items[i].0,
                            reduce_items[i].1,
                            reduce_items[j].1,
                        );
                        unresolved_conflicts.push(conflict);
                    }
                }
            }
        }

        (unresolved_conflicts, resolved_conflicts)
    }
}

#[derive(Debug, Clone)]
pub struct LR1Table {
    pub action: ActionTable,
    pub goto: GotoTable,
    pub state_count: usize,
    pub terminal_count: usize,
    pub non_terminal_count: usize,
    pub conflicts: Vec<Conflict>,
}

impl LR1Table {
    pub fn new(state_count: usize, terminal_count: usize, non_terminal_count: usize) -> Self {
        LR1Table {
            action: ActionTable::new(state_count, terminal_count),
            goto: GotoTable::new(state_count, non_terminal_count),
            state_count,
            terminal_count,
            non_terminal_count,
            conflicts: Vec::new(),
        }
    }

    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    pub fn conflict_report(&self) -> ConflictReport {
        ConflictReport::new(self.conflicts.clone())
    }

    pub fn grammar_type(&self) -> GrammarType {
        if self.conflicts.is_empty() {
            GrammarType::LR1
        } else {
            GrammarType::NotLR
        }
    }

    pub fn is_lr1(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn print(&self, cfg: &ContextFreeGrammar) {
        println!("LR(1) Parsing Table:");
        println!();
        
        let terminal_names: Vec<String> = (0..cfg.terminals.len())
            .map(|i| cfg.get_terminal_name(crate::cfg::TerminalId(i)).to_string())
            .collect();
        let non_terminal_names: Vec<String> = (0..cfg.non_terminals.len())
            .map(|i| cfg.get_non_terminal_name(crate::cfg::NonTerminalId(i)).to_string())
            .collect();
        
        self.action.print(&terminal_names);
        println!();
        self.goto.print(&non_terminal_names);
        
        if self.has_conflicts() {
            println!();
            println!("Conflicts:");
            for conflict in &self.conflicts {
                println!("  {}", conflict);
            }
        }
    }
}

impl fmt::Display for LR1Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "LR(1) Parsing Table:")?;
        writeln!(f, "{}", self.action)?;
        writeln!(f, "{}", self.goto)?;
        if self.has_conflicts() {
            writeln!(f, "\nConflicts:")?;
            for conflict in &self.conflicts {
                writeln!(f, "  {}", conflict)?;
            }
        }
        Ok(())
    }
}

pub struct LR1TableBuilder;

impl LR1TableBuilder {
    pub fn build(augmented: &AugmentedGrammar) -> LR1Table {
        let cfg = augmented.grammar();
        let calculator = LookaheadCalculator::new(cfg);
        let collection = LR1ItemSetCollection::build_from_augmented(augmented, &calculator);
        Self::build_from_collection(augmented, &collection, &calculator)
    }

    pub fn build_from_collection(
        augmented: &AugmentedGrammar,
        collection: &LR1ItemSetCollection,
        calculator: &LookaheadCalculator,
    ) -> LR1Table {
        let cfg = augmented.grammar();
        let state_count = collection.len();
        let terminal_count = cfg.terminals.len();
        let non_terminal_count = cfg.non_terminals.len();
        let end_marker = calculator.end_marker();

        let mut table = LR1Table::new(state_count, terminal_count, non_terminal_count);

        for (state_idx, item_set) in collection.states().iter().enumerate() {
            let state_id = ItemSetId(state_idx);

            for item in item_set.items() {
                if let Some(production) = cfg.productions.get(item.production_index) {
                    if item.is_complete(production) {
                        if augmented.is_accepting_production(item.production_index) {
                            if item.lookahead == end_marker {
                                table.action.set_accept(state_id, end_marker);
                            }
                        } else {
                            table.action.set_reduce(
                                state_id,
                                item.lookahead,
                                ProductionId(item.production_index),
                            );
                        }
                    } else if let Some(Symbol::Terminal(term_id)) = item.next_symbol(production) {
                        if let Some(next_state) =
                            collection.get_transition(state_idx, &Symbol::Terminal(*term_id))
                        {
                            table.action.set_shift(state_id, *term_id, ItemSetId(next_state));
                        }
                    } else if let Some(Symbol::NonTerminal(nt_id)) = item.next_symbol(production) {
                        if let Some(next_state) =
                            collection.get_transition(state_idx, &Symbol::NonTerminal(*nt_id))
                        {
                            table.goto.set(state_id, *nt_id, ItemSetId(next_state));
                        }
                    }
                }
            }
        }

        table.conflicts = Self::detect_conflicts(&table, collection, augmented, end_marker);
        table
    }

    fn detect_conflicts(
        _table: &LR1Table,
        collection: &LR1ItemSetCollection,
        augmented: &AugmentedGrammar,
        end_marker: TerminalId,
    ) -> Vec<Conflict> {
        let mut conflicts = Vec::new();
        let cfg = augmented.grammar();

        for (state_id, item_set) in collection.states().iter().enumerate() {
            let mut shift_terms: HashMap<TerminalId, ItemSetId> = HashMap::new();
            let mut reduce_terms: HashMap<TerminalId, ProductionId> = HashMap::new();

            for item in item_set.items() {
                if let Some(production) = cfg.productions.get(item.production_index) {
                    if item.is_complete(production) {
                        if !augmented.is_accepting_production(item.production_index)
                            || item.lookahead != end_marker
                        {
                            if let Some(existing) = reduce_terms.get(&item.lookahead) {
                                if *existing != ProductionId(item.production_index) {
                                    conflicts.push(Conflict::reduce_reduce(
                                        ItemSetId(state_id),
                                        item.lookahead,
                                        *existing,
                                        ProductionId(item.production_index),
                                    ));
                                }
                            } else {
                                reduce_terms.insert(item.lookahead, ProductionId(item.production_index));
                            }
                        }
                    } else if let Some(Symbol::Terminal(term_id)) = item.next_symbol(production) {
                        if let Some(next_state) =
                            collection.get_transition(state_id, &Symbol::Terminal(*term_id))
                        {
                            shift_terms.insert(*term_id, ItemSetId(next_state));
                        }
                    }
                }
            }

            for (term, shift_state) in &shift_terms {
                if let Some(prod_id) = reduce_terms.get(term) {
                    conflicts.push(Conflict::shift_reduce(
                        ItemSetId(state_id),
                        *term,
                        *shift_state,
                        *prod_id,
                    ));
                }
            }
        }

        conflicts
    }
}

#[derive(Debug, Clone)]
pub struct LALR1Table {
    pub action: ActionTable,
    pub goto: GotoTable,
    pub state_count: usize,
    pub terminal_count: usize,
    pub non_terminal_count: usize,
    pub conflicts: Vec<Conflict>,
    pub merged_states: usize,
}

impl LALR1Table {
    pub fn new(state_count: usize, terminal_count: usize, non_terminal_count: usize) -> Self {
        LALR1Table {
            action: ActionTable::new(state_count, terminal_count),
            goto: GotoTable::new(state_count, non_terminal_count),
            state_count,
            terminal_count,
            non_terminal_count,
            conflicts: Vec::new(),
            merged_states: 0,
        }
    }

    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }

    pub fn conflict_report(&self) -> ConflictReport {
        ConflictReport::new(self.conflicts.clone())
    }

    pub fn grammar_type(&self) -> GrammarType {
        if self.conflicts.is_empty() {
            GrammarType::LALR1
        } else {
            GrammarType::NotLR
        }
    }

    pub fn is_lalr1(&self) -> bool {
        self.conflicts.is_empty()
    }

    pub fn print(&self, cfg: &ContextFreeGrammar) {
        println!("LALR(1) Parsing Table ({} merged states):", self.merged_states);
        println!();
        
        let terminal_names: Vec<String> = (0..cfg.terminals.len())
            .map(|i| cfg.get_terminal_name(crate::cfg::TerminalId(i)).to_string())
            .collect();
        let non_terminal_names: Vec<String> = (0..cfg.non_terminals.len())
            .map(|i| cfg.get_non_terminal_name(crate::cfg::NonTerminalId(i)).to_string())
            .collect();
        
        self.action.print(&terminal_names);
        println!();
        self.goto.print(&non_terminal_names);
        
        if self.has_conflicts() {
            println!();
            println!("Conflicts introduced by merging:");
            for conflict in &self.conflicts {
                println!("  {}", conflict);
            }
        }
    }
}

impl fmt::Display for LALR1Table {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "LALR(1) Parsing Table ({} merged states):", self.merged_states)?;
        writeln!(f, "{}", self.action)?;
        writeln!(f, "{}", self.goto)?;
        if self.has_conflicts() {
            writeln!(f, "\nConflicts introduced by merging:")?;
            for conflict in &self.conflicts {
                writeln!(f, "  {}", conflict)?;
            }
        }
        Ok(())
    }
}

pub struct LALR1TableBuilder;

impl LALR1TableBuilder {
    pub fn build(augmented: &AugmentedGrammar) -> LALR1Table {
        let cfg = augmented.grammar();
        let calculator = LookaheadCalculator::new(cfg);
        let lr1_collection = LR1ItemSetCollection::build_from_augmented(augmented, &calculator);
        Self::build_from_lr1_collection(augmented, &lr1_collection, &calculator)
    }

    pub fn build_from_lr1_collection(
        augmented: &AugmentedGrammar,
        lr1_collection: &LR1ItemSetCollection,
        calculator: &LookaheadCalculator,
    ) -> LALR1Table {
        let cfg = augmented.grammar();
        let end_marker = calculator.end_marker();

        let mut merger = CoreSetMerger::new();
        let mut state_mapping: Vec<usize> = Vec::new();

        for item_set in lr1_collection.states() {
            let merged_id = merger.add_or_merge(item_set.items().clone());
            state_mapping.push(merged_id);
        }

        let merged_states = merger.into_states();
        let original_count = lr1_collection.len();
        let merged_count = merged_states.len();

        let mut table = LALR1Table::new(merged_count, cfg.terminals.len(), cfg.non_terminals.len());
        table.merged_states = original_count - merged_count;

        let mut merged_to_rep: Vec<usize> = vec![0; merged_count];
        for (orig_id, &merged_id) in state_mapping.iter().enumerate() {
            merged_to_rep[merged_id] = orig_id;
        }

        for (merged_id, item_set) in merged_states.iter().enumerate() {
            let state_id = ItemSetId(merged_id);
            let rep_orig_id = merged_to_rep[merged_id];

            for item in item_set {
                if let Some(production) = cfg.productions.get(item.production_index) {
                    if item.is_complete(production) {
                        if augmented.is_accepting_production(item.production_index) {
                            if item.lookahead == end_marker {
                                table.action.set_accept(state_id, end_marker);
                            }
                        } else {
                            table.action.set_reduce(
                                state_id,
                                item.lookahead,
                                ProductionId(item.production_index),
                            );
                        }
                    } else if let Some(Symbol::Terminal(term_id)) = item.next_symbol(production) {
                        if let Some(next_orig) = lr1_collection
                            .get_transition(rep_orig_id, &Symbol::Terminal(*term_id))
                        {
                            let next_merged = state_mapping[next_orig];
                            table.action.set_shift(state_id, *term_id, ItemSetId(next_merged));
                        }
                    } else if let Some(Symbol::NonTerminal(nt_id)) = item.next_symbol(production) {
                        if let Some(next_orig) = lr1_collection
                            .get_transition(rep_orig_id, &Symbol::NonTerminal(*nt_id))
                        {
                            let next_merged = state_mapping[next_orig];
                            table.goto.set(state_id, *nt_id, ItemSetId(next_merged));
                        }
                    }
                }
            }
        }

        table.conflicts = Self::detect_conflicts(&table, &merged_states, augmented, end_marker);
        table
    }

    fn detect_conflicts(
        table: &LALR1Table,
        merged_states: &[HashSet<crate::lr::item::LR1Item>],
        augmented: &AugmentedGrammar,
        end_marker: TerminalId,
    ) -> Vec<Conflict> {
        let mut conflicts = Vec::new();
        let cfg = augmented.grammar();

        for (state_id, item_set) in merged_states.iter().enumerate() {
            let mut shift_terms: HashMap<TerminalId, ItemSetId> = HashMap::new();
            let mut reduce_terms: HashMap<TerminalId, ProductionId> = HashMap::new();

            for item in item_set {
                if let Some(production) = cfg.productions.get(item.production_index) {
                    if item.is_complete(production) {
                        if !augmented.is_accepting_production(item.production_index)
                            || item.lookahead != end_marker
                        {
                            if let Some(existing) = reduce_terms.get(&item.lookahead) {
                                if *existing != ProductionId(item.production_index) {
                                    conflicts.push(Conflict::reduce_reduce(
                                        ItemSetId(state_id),
                                        item.lookahead,
                                        *existing,
                                        ProductionId(item.production_index),
                                    ));
                                }
                            } else {
                                reduce_terms.insert(item.lookahead, ProductionId(item.production_index));
                            }
                        }
                    }
                }
            }

            for term_idx in 0..=cfg.terminals.len() {
                let terminal = TerminalId(term_idx);
                let action = table.action.get(ItemSetId(state_id), terminal);
                if let Action::Shift(shift_state) = action {
                    shift_terms.insert(terminal, *shift_state);
                }
            }

            for (term, shift_state) in &shift_terms {
                if let Some(prod_id) = reduce_terms.get(term) {
                    conflicts.push(Conflict::shift_reduce(
                        ItemSetId(state_id),
                        *term,
                        *shift_state,
                        *prod_id,
                    ));
                }
            }
        }

        conflicts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::ContextFreeGrammar;

    fn parse_grammar(input: &str) -> ContextFreeGrammar {
        ContextFreeGrammar::parse(input).unwrap()
    }

    #[test]
    fn test_lr0_table_creation() {
        let table = LR0Table::new(5, 3, 2);

        assert_eq!(table.state_count, 5);
        assert_eq!(table.terminal_count, 3);
        assert_eq!(table.non_terminal_count, 2);
        assert!(!table.has_conflicts());
    }

    #[test]
    fn test_lr0_table_builder_simple() {
        let grammar = parse_grammar("E -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let table = LR0TableBuilder::build(&augmented);

        assert!(table.state_count > 0);
        assert!(table.is_lr0());
    }

    #[test]
    fn test_slr_table_creation() {
        let table = SLRTable::new(5, 3, 2);

        assert_eq!(table.state_count, 5);
        assert_eq!(table.terminal_count, 3);
        assert_eq!(table.non_terminal_count, 2);
        assert!(!table.has_conflicts());
    }

    #[test]
    fn test_slr_table_builder_simple() {
        let grammar = parse_grammar("E -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let table = SLRTableBuilder::build(&augmented);

        assert!(table.state_count > 0);
        assert!(table.is_lr0());
    }

    #[test]
    fn test_grammar_type_display() {
        assert_eq!(format!("{}", GrammarType::LR0), "LR(0)");
        assert_eq!(format!("{}", GrammarType::SLR1), "SLR(1)");
        assert_eq!(format!("{}", GrammarType::LR1), "LR(1)");
        assert_eq!(format!("{}", GrammarType::LALR1), "LALR(1)");
        assert_eq!(format!("{}", GrammarType::NotLR), "Not LR");
    }

    #[test]
    fn test_lr1_table_builder_simple() {
        let grammar = parse_grammar("E -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let table = LR1TableBuilder::build(&augmented);

        assert!(table.state_count > 0);
        assert!(table.is_lr1());
    }

    #[test]
    fn test_lalr1_table_builder_simple() {
        let grammar = parse_grammar("E -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let table = LALR1TableBuilder::build(&augmented);

        assert!(table.state_count > 0);
        assert!(table.is_lalr1());
    }

    #[test]
    fn test_lr1_table_expression() {
        let grammar = parse_grammar("E -> E + T | T\nT -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let table = LR1TableBuilder::build(&augmented);

        assert!(table.is_lr1());
    }

    #[test]
    fn test_lalr1_table_expression() {
        let grammar = parse_grammar("E -> E + T | T\nT -> num");
        let augmented = AugmentedGrammar::new(grammar);

        let table = LALR1TableBuilder::build(&augmented);

        assert!(table.is_lalr1());
    }
}
