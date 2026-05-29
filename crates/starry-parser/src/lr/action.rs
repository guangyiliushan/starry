use crate::cfg::TerminalId;
use crate::lr::goto::ItemSetId;
use std::fmt;

/// 产生式 ID
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProductionId(pub usize);

/// ACTION 表条目
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// 移进：将当前终结符压栈，转到状态 state
    Shift(ItemSetId),
    /// 归约：使用产生式 production 进行归约
    Reduce(ProductionId),
    /// 接受：解析成功完成
    Accept,
    /// 错误：无合法动作
    Error,
}

impl Action {
    pub fn shift(state: usize) -> Self {
        Action::Shift(ItemSetId(state))
    }

    pub fn reduce(production: usize) -> Self {
        Action::Reduce(ProductionId(production))
    }

    pub fn accept() -> Self {
        Action::Accept
    }

    pub fn error() -> Self {
        Action::Error
    }

    pub fn is_shift(&self) -> bool {
        matches!(self, Action::Shift(_))
    }

    pub fn is_reduce(&self) -> bool {
        matches!(self, Action::Reduce(_))
    }

    pub fn is_accept(&self) -> bool {
        matches!(self, Action::Accept)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Action::Error)
    }

    pub fn shift_state(&self) -> Option<ItemSetId> {
        match self {
            Action::Shift(state) => Some(*state),
            _ => None,
        }
    }

    pub fn reduce_production(&self) -> Option<ProductionId> {
        match self {
            Action::Reduce(prod) => Some(*prod),
            _ => None,
        }
    }
}

impl Default for Action {
    fn default() -> Self {
        Action::Error
    }
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Shift(state) => write!(f, "s{}", state.0),
            Action::Reduce(prod) => write!(f, "r{}", prod.0),
            Action::Accept => write!(f, "acc"),
            Action::Error => write!(f, ""),
        }
    }
}

/// ACTION 表
#[derive(Debug, Clone)]
pub struct ActionTable {
    actions: Vec<Vec<Action>>,
    terminal_count: usize,
}

impl ActionTable {
    pub fn new(state_count: usize, terminal_count: usize) -> Self {
        ActionTable {
            actions: vec![vec![Action::Error; terminal_count + 1]; state_count],
            terminal_count,
        }
    }

    pub fn get(&self, state: ItemSetId, terminal: TerminalId) -> &Action {
        &self.actions[state.0][terminal.0]
    }

    pub fn set(&mut self, state: ItemSetId, terminal: TerminalId, action: Action) {
        self.actions[state.0][terminal.0] = action;
    }

    pub fn set_shift(&mut self, state: ItemSetId, terminal: TerminalId, next_state: ItemSetId) {
        self.actions[state.0][terminal.0] = Action::Shift(next_state);
    }

    pub fn set_reduce(&mut self, state: ItemSetId, terminal: TerminalId, production: ProductionId) {
        self.actions[state.0][terminal.0] = Action::Reduce(production);
    }

    pub fn set_accept(&mut self, state: ItemSetId, terminal: TerminalId) {
        self.actions[state.0][terminal.0] = Action::Accept;
    }

    pub fn state_count(&self) -> usize {
        self.actions.len()
    }

    pub fn terminal_count(&self) -> usize {
        self.terminal_count
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, usize, &Action)> {
        self.actions.iter().enumerate().flat_map(|(state, row)| {
            row.iter().enumerate().map(move |(term, action)| (state, term, action))
        })
    }

    pub fn print(&self, terminal_names: &[String]) {
        let term_count = self.terminal_count;
        let col_widths = self.calculate_column_widths(terminal_names);
        
        println!("ACTION Table:");
        
        print!("{:>w$} |", "State", w = 5);
        for t in 0..=term_count {
            let header = if t < term_count {
                terminal_names.get(t).cloned().unwrap_or_else(|| format!("T{}", t))
            } else {
                "$".to_string()
            };
            print!(" {:>w$} |", header, w = col_widths[t]);
        }
        println!();
        
        print!("{:-<5}|", "");
        for t in 0..=term_count {
            print!("{:-<w$}|", "", w = col_widths[t] + 2);
        }
        println!();
        
        for (state, row) in self.actions.iter().enumerate() {
            print!("{:>5} |", state);
            for (t, action) in row.iter().enumerate() {
                let action_str = format!("{}", action);
                print!(" {:>w$} |", action_str, w = col_widths[t]);
            }
            println!();
        }
    }

    fn calculate_column_widths(&self, terminal_names: &[String]) -> Vec<usize> {
        let mut widths = Vec::with_capacity(self.terminal_count + 1);
        
        for t in 0..=self.terminal_count {
            let header_width = if t < self.terminal_count {
                terminal_names.get(t).map(|s| s.len()).unwrap_or(2)
            } else {
                1
            };
            
            let mut max_action_width = 0;
            for row in &self.actions {
                if let Some(action) = row.get(t) {
                    let action_str = format!("{}", action);
                    max_action_width = max_action_width.max(action_str.len());
                }
            }
            
            widths.push(header_width.max(max_action_width));
        }
        
        widths
    }
}

impl fmt::Display for ActionTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ACTION Table:")?;
        write!(f, "     ")?;
        for t in 0..self.terminal_count + 1 {
            if t < self.terminal_count {
                write!(f, "{:4} ", t)?;
            } else {
                write!(f, "  $  ")?;
            }
        }
        writeln!(f)?;

        for (state, row) in self.actions.iter().enumerate() {
            write!(f, "{:3}: ", state)?;
            for action in row {
                write!(f, "{:4} ", action)?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_creation() {
        let shift = Action::shift(1);
        assert!(shift.is_shift());
        assert_eq!(shift.shift_state(), Some(ItemSetId(1)));

        let reduce = Action::reduce(2);
        assert!(reduce.is_reduce());
        assert_eq!(reduce.reduce_production(), Some(ProductionId(2)));

        let accept = Action::accept();
        assert!(accept.is_accept());

        let error = Action::error();
        assert!(error.is_error());
    }

    #[test]
    fn test_action_table() {
        let mut table = ActionTable::new(3, 2);

        table.set_shift(ItemSetId(0), TerminalId(0), ItemSetId(1));
        table.set_reduce(ItemSetId(1), TerminalId(1), ProductionId(0));
        table.set_accept(ItemSetId(2), TerminalId(2));

        assert!(table.get(ItemSetId(0), TerminalId(0)).is_shift());
        assert!(table.get(ItemSetId(1), TerminalId(1)).is_reduce());
        assert!(table.get(ItemSetId(2), TerminalId(2)).is_accept());
    }

    #[test]
    fn test_action_display() {
        assert_eq!(format!("{}", Action::shift(1)), "s1");
        assert_eq!(format!("{}", Action::reduce(2)), "r2");
        assert_eq!(format!("{}", Action::accept()), "acc");
        assert_eq!(format!("{}", Action::error()), "");
    }
}
