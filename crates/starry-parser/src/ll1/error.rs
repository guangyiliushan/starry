use std::fmt;

#[derive(Debug)]
pub struct LL1ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub expected: Vec<String>,
    pub found: Option<String>,
}

impl LL1ParseError {
    pub fn new(message: String, line: usize, column: usize) -> Self {
        LL1ParseError {
            message,
            line,
            column,
            expected: Vec::new(),
            found: None,
        }
    }

    pub fn with_expected(mut self, expected: Vec<String>) -> Self {
        self.expected = expected;
        self
    }

    pub fn with_found(mut self, found: Option<String>) -> Self {
        self.found = found;
        self
    }

    pub fn unexpected_input(line: usize, column: usize, found: String) -> Self {
        Self::new(
            "Unexpected input after parsing completed".to_string(),
            line,
            column,
        )
        .with_found(Some(found))
    }

    pub fn unexpected_eof(expected: String) -> Self {
        Self::new(
            format!("Unexpected end of input, expected '{}'", expected),
            0,
            0,
        )
        .with_expected(vec![expected])
    }

    pub fn terminal_mismatch(expected: String, found: String, line: usize, column: usize) -> Self {
        Self::new(
            format!("Expected '{}', found '{}'", expected, found),
            line,
            column,
        )
        .with_expected(vec![expected])
        .with_found(Some(found))
    }

    pub fn no_production(non_terminal: String, line: usize, column: usize, expected: Vec<String>, found: Option<String>) -> Self {
        Self::new(
            format!("No production for {} at current input", non_terminal),
            line,
            column,
        )
        .with_expected(expected)
        .with_found(found)
    }

    pub fn unknown_terminal(terminal: String, line: usize, column: usize) -> Self {
        Self::new(
            format!("Unknown terminal: {}", terminal),
            line,
            column,
        )
    }

    pub fn stack_underflow() -> Self {
        Self::new(
            "Stack underflow - this should not happen".to_string(),
            0,
            0,
        )
    }
}

impl fmt::Display for LL1ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Parse error at line {}, column {}: {}",
            self.line, self.column, self.message
        )?;

        if !self.expected.is_empty() {
            write!(f, "\n  Expected one of: {}", self.expected.join(", "))?;
        }

        if let Some(ref found) = self.found {
            write!(f, "\n  Found: {}", found)?;
        }

        Ok(())
    }
}

impl std::error::Error for LL1ParseError {}
