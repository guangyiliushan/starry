use crate::cfg::TerminalId;
use crate::lr::grammar_type::LRGrammarType;
use crate::lr::{Conflict, ItemSetId};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LRErrorKind {
    UnexpectedToken {
        expected: Vec<TerminalId>,
        found: TerminalId,
        state: ItemSetId,
    },
    UnexpectedEndOfInput {
        expected: Vec<TerminalId>,
        state: ItemSetId,
    },
    NoAction {
        state: ItemSetId,
        terminal: TerminalId,
    },
    ConflictDetected {
        conflicts: Vec<Conflict>,
    },
    InvalidGrammar {
        message: String,
    },
    StackUnderflow,
    InvalidState {
        state: ItemSetId,
    },
}

impl fmt::Display for LRErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LRErrorKind::UnexpectedToken { expected, found, state } => {
                write!(
                    f,
                    "Unexpected token {} in state {}. Expected one of: {:?}",
                    found.0, state.0, expected
                )
            }
            LRErrorKind::UnexpectedEndOfInput { expected, state } => {
                write!(
                    f,
                    "Unexpected end of input in state {}. Expected one of: {:?}",
                    state.0, expected
                )
            }
            LRErrorKind::NoAction { state, terminal } => {
                write!(
                    f,
                    "No action defined for terminal {} in state {}",
                    terminal.0, state.0
                )
            }
            LRErrorKind::ConflictDetected { conflicts } => {
                writeln!(f, "Grammar has {} conflict(s):", conflicts.len())?;
                for conflict in conflicts {
                    writeln!(f, "  {}", conflict)?;
                }
                Ok(())
            }
            LRErrorKind::InvalidGrammar { message } => {
                write!(f, "Invalid grammar: {}", message)
            }
            LRErrorKind::StackUnderflow => {
                write!(f, "Parser stack underflow")
            }
            LRErrorKind::InvalidState { state } => {
                write!(f, "Invalid state: {}", state.0)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LRError {
    pub kind: LRErrorKind,
    pub position: Option<usize>,
    pub context: Option<String>,
}

impl LRError {
    pub fn new(kind: LRErrorKind) -> Self {
        LRError {
            kind,
            position: None,
            context: None,
        }
    }

    pub fn unexpected_token(expected: Vec<TerminalId>, found: TerminalId, state: ItemSetId) -> Self {
        LRError::new(LRErrorKind::UnexpectedToken { expected, found, state })
    }

    pub fn unexpected_end(expected: Vec<TerminalId>, state: ItemSetId) -> Self {
        LRError::new(LRErrorKind::UnexpectedEndOfInput { expected, state })
    }

    pub fn no_action(state: ItemSetId, terminal: TerminalId) -> Self {
        LRError::new(LRErrorKind::NoAction { state, terminal })
    }

    pub fn conflict(conflicts: Vec<Conflict>) -> Self {
        LRError::new(LRErrorKind::ConflictDetected { conflicts })
    }

    pub fn invalid_grammar(message: impl Into<String>) -> Self {
        LRError::new(LRErrorKind::InvalidGrammar { message: message.into() })
    }

    pub fn stack_underflow() -> Self {
        LRError::new(LRErrorKind::StackUnderflow)
    }

    pub fn invalid_state(state: ItemSetId) -> Self {
        LRError::new(LRErrorKind::InvalidState { state })
    }

    pub fn with_position(mut self, position: usize) -> Self {
        self.position = Some(position);
        self
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn is_unexpected_token(&self) -> bool {
        matches!(self.kind, LRErrorKind::UnexpectedToken { .. })
    }

    pub fn is_conflict(&self) -> bool {
        matches!(self.kind, LRErrorKind::ConflictDetected { .. })
    }

    pub fn is_invalid_grammar(&self) -> bool {
        matches!(self.kind, LRErrorKind::InvalidGrammar { .. })
    }
}

impl fmt::Display for LRError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "LR Parse Error: {}", self.kind)?;
        if let Some(pos) = self.position {
            write!(f, " at position {}", pos)?;
        }
        if let Some(ctx) = &self.context {
            write!(f, "\nContext: {}", ctx)?;
        }
        Ok(())
    }
}

impl std::error::Error for LRError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LRValidationError {
    ShiftReduceConflict {
        state: ItemSetId,
        terminal: TerminalId,
    },
    ReduceReduceConflict {
        state: ItemSetId,
        terminal: TerminalId,
    },
    NotLR0,
    NotSLR1,
    NotLALR1,
    NotLR1,
}

impl fmt::Display for LRValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LRValidationError::ShiftReduceConflict { state, terminal } => {
                write!(
                    f,
                    "Shift/Reduce conflict in state {} on terminal {}",
                    state.0, terminal.0
                )
            }
            LRValidationError::ReduceReduceConflict { state, terminal } => {
                write!(
                    f,
                    "Reduce/Reduce conflict in state {} on terminal {}",
                    state.0, terminal.0
                )
            }
            LRValidationError::NotLR0 => write!(f, "Grammar is not LR(0)"),
            LRValidationError::NotSLR1 => write!(f, "Grammar is not SLR(1)"),
            LRValidationError::NotLALR1 => write!(f, "Grammar is not LALR(1)"),
            LRValidationError::NotLR1 => write!(f, "Grammar is not LR(1)"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct LRValidationReport {
    pub is_valid: bool,
    pub errors: Vec<LRValidationError>,
    pub warnings: Vec<String>,
    pub grammar_type: LRGrammarType,
}

impl LRValidationReport {
    pub fn new() -> Self {
        LRValidationReport {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            grammar_type: LRGrammarType::Unknown,
        }
    }

    pub fn add_error(&mut self, error: LRValidationError) {
        self.is_valid = false;
        self.errors.push(error);
    }

    pub fn add_warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }

    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

impl Default for LRValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for LRValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "LR Grammar Validation Report")?;
        writeln!(f, "  Grammar Type: {:?}", self.grammar_type)?;
        writeln!(f, "  Valid: {}", self.is_valid)?;

        if !self.errors.is_empty() {
            writeln!(f, "  Errors:")?;
            for error in &self.errors {
                writeln!(f, "    - {}", error)?;
            }
        }

        if !self.warnings.is_empty() {
            writeln!(f, "  Warnings:")?;
            for warning in &self.warnings {
                writeln!(f, "    - {}", warning)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lr_error_creation() {
        let error = LRError::unexpected_token(
            vec![TerminalId(0), TerminalId(1)],
            TerminalId(2),
            ItemSetId(0),
        );

        assert!(error.is_unexpected_token());
        assert!(error.position.is_none());
    }

    #[test]
    fn test_lr_error_with_position() {
        let error = LRError::unexpected_token(
            vec![TerminalId(0)],
            TerminalId(1),
            ItemSetId(0),
        ).with_position(5);

        assert_eq!(error.position, Some(5));
    }

    #[test]
    fn test_validation_report() {
        let mut report = LRValidationReport::new();
        assert!(report.is_valid);

        report.add_error(LRValidationError::NotSLR1);
        assert!(!report.is_valid);
        assert!(report.has_errors());
    }

    #[test]
    fn test_grammar_type() {
        assert!(LRGrammarType::LR0.is_deterministic());
        assert!(LRGrammarType::SLR1.is_slr1());
        assert!(!LRGrammarType::NotLR.is_deterministic());

        assert_eq!(LRGrammarType::LALR1.name(), "LALR(1)");
    }
}
