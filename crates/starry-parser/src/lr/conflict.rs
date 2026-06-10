use crate::cfg::TerminalId;
use crate::lr::action::{Action, ProductionId};
use crate::lr::goto::ItemSetId;
use std::fmt;

/// 冲突类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictKind {
    /// 移进-归约冲突：在同一终结符上既可以移进也可以归约
    ShiftReduce {
        terminal: TerminalId,
        shift_state: ItemSetId,
        reduce_production: ProductionId,
    },
    /// 归约-归约冲突：在同一终结符上可以用不同产生式归约
    ReduceReduce {
        terminal: TerminalId,
        production1: ProductionId,
        production2: ProductionId,
    },
}

/// 冲突描述
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub state: ItemSetId,
    pub kind: ConflictKind,
}

impl Conflict {
    pub fn shift_reduce(
        state: ItemSetId,
        terminal: TerminalId,
        shift_state: ItemSetId,
        reduce_production: ProductionId,
    ) -> Self {
        Conflict {
            state,
            kind: ConflictKind::ShiftReduce {
                terminal,
                shift_state,
                reduce_production,
            },
        }
    }

    pub fn reduce_reduce(
        state: ItemSetId,
        terminal: TerminalId,
        production1: ProductionId,
        production2: ProductionId,
    ) -> Self {
        Conflict {
            state,
            kind: ConflictKind::ReduceReduce {
                terminal,
                production1,
                production2,
            },
        }
    }

    pub fn is_shift_reduce(&self) -> bool {
        matches!(self.kind, ConflictKind::ShiftReduce { .. })
    }

    pub fn is_reduce_reduce(&self) -> bool {
        matches!(self.kind, ConflictKind::ReduceReduce { .. })
    }
}

impl fmt::Display for Conflict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "State {}: ", self.state.0)?;
        match &self.kind {
            ConflictKind::ShiftReduce {
                terminal,
                shift_state,
                reduce_production,
            } => {
                write!(
                    f,
                    "Shift/Reduce conflict on terminal {}: shift to state {} or reduce by production {}",
                    terminal.0, shift_state.0, reduce_production.0
                )
            }
            ConflictKind::ReduceReduce {
                terminal,
                production1,
                production2,
            } => {
                write!(
                    f,
                    "Reduce/Reduce conflict on terminal {}: reduce by production {} or {}",
                    terminal.0, production1.0, production2.0
                )
            }
        }
    }
}

/// 冲突检测器
pub struct ConflictDetector;

impl ConflictDetector {
    pub fn detect(
        state: ItemSetId,
        terminal: TerminalId,
        existing: &Action,
        new: &Action,
    ) -> Option<Conflict> {
        match (existing, new) {
            (Action::Shift(s1), Action::Shift(s2)) if s1 != s2 => {
                None
            }
            (Action::Shift(shift_state), Action::Reduce(prod)) => {
                Some(Conflict::shift_reduce(
                    state,
                    terminal,
                    *shift_state,
                    *prod,
                ))
            }
            (Action::Reduce(prod), Action::Shift(shift_state)) => {
                Some(Conflict::shift_reduce(
                    state,
                    terminal,
                    *shift_state,
                    *prod,
                ))
            }
            (Action::Reduce(p1), Action::Reduce(p2)) if p1 != p2 => {
                Some(Conflict::reduce_reduce(state, terminal, *p1, *p2))
            }
            _ => None,
        }
    }

    pub fn detect_all(actions: &[(ItemSetId, TerminalId, Action, Action)]) -> Vec<Conflict> {
        actions
            .iter()
            .filter_map(|(state, terminal, existing, new)| {
                Self::detect(*state, *terminal, existing, new)
            })
            .collect()
    }
}

/// 冲突报告
#[derive(Debug, Clone)]
pub struct ConflictReport {
    pub conflicts: Vec<Conflict>,
    pub shift_reduce_count: usize,
    pub reduce_reduce_count: usize,
}

impl ConflictReport {
    pub fn new(conflicts: Vec<Conflict>) -> Self {
        let shift_reduce_count = conflicts.iter().filter(|c| c.is_shift_reduce()).count();
        let reduce_reduce_count = conflicts.iter().filter(|c| c.is_reduce_reduce()).count();

        ConflictReport {
            conflicts,
            shift_reduce_count,
            reduce_reduce_count,
        }
    }

    pub fn is_ambiguous(&self) -> bool {
        !self.conflicts.is_empty()
    }

    pub fn has_shift_reduce_conflicts(&self) -> bool {
        self.shift_reduce_count > 0
    }

    pub fn has_reduce_reduce_conflicts(&self) -> bool {
        self.reduce_reduce_count > 0
    }
}

impl fmt::Display for ConflictReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.conflicts.is_empty() {
            writeln!(f, "No conflicts detected.")?;
        } else {
            writeln!(
                f,
                "Detected {} conflict(s):",
                self.conflicts.len()
            )?;
            writeln!(f, "  Shift/Reduce: {}", self.shift_reduce_count)?;
            writeln!(f, "  Reduce/Reduce: {}", self.reduce_reduce_count)?;
            writeln!(f)?;
            for conflict in &self.conflicts {
                writeln!(f, "  {}", conflict)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shift_reduce_conflict() {
        let conflict = Conflict::shift_reduce(
            ItemSetId(0),
            TerminalId(0),
            ItemSetId(1),
            ProductionId(2),
        );

        assert!(conflict.is_shift_reduce());
        assert!(!conflict.is_reduce_reduce());
    }

    #[test]
    fn test_reduce_reduce_conflict() {
        let conflict = Conflict::reduce_reduce(
            ItemSetId(0),
            TerminalId(0),
            ProductionId(1),
            ProductionId(2),
        );

        assert!(conflict.is_reduce_reduce());
        assert!(!conflict.is_shift_reduce());
    }

    #[test]
    fn test_conflict_detection() {
        let shift = Action::shift(1);
        let reduce = Action::reduce(2);

        let conflict = ConflictDetector::detect(
            ItemSetId(0),
            TerminalId(0),
            &shift,
            &reduce,
        );

        assert!(conflict.is_some());
        let c = conflict.unwrap();
        assert!(c.is_shift_reduce());
    }

    #[test]
    fn test_conflict_report() {
        let conflicts = vec![
            Conflict::shift_reduce(ItemSetId(0), TerminalId(0), ItemSetId(1), ProductionId(0)),
            Conflict::reduce_reduce(ItemSetId(1), TerminalId(1), ProductionId(0), ProductionId(1)),
        ];

        let report = ConflictReport::new(conflicts);

        assert!(report.is_ambiguous());
        assert!(report.has_shift_reduce_conflicts());
        assert!(report.has_reduce_reduce_conflicts());
        assert_eq!(report.shift_reduce_count, 1);
        assert_eq!(report.reduce_reduce_count, 1);
    }
}
