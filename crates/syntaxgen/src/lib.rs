//! Single-source lexical vocabulary generator for Starry.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const LEXICON_PATH: &str = "docs/syntax/lexicon.txt";
pub const KEYWORD_GEN_PATH: &str = "crates/ast/src/token/keyword_gen.rs";
pub const ANTLR_TERMS_PATH: &str = "docs/syntax/generated/StarryLexerTerms.g4";
pub const MARKDOWN_TABLE_PATH: &str = "docs/syntax/generated/lexicon.md";

pub const HARD_KEYWORD_COUNT: usize = 39;
pub const MODIFIER_WORD_COUNT: usize = 14;
pub const SLOT_WORD_COUNT: usize = 31;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WordKind {
    Hard,
    Modifier,
    Slot,
}

impl WordKind {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "hard" => Some(Self::Hard),
            "modifier" => Some(Self::Modifier),
            "slot" => Some(Self::Slot),
            _ => None,
        }
    }

    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Self::Hard => "Hard keyword",
            Self::Modifier => "Modifier word",
            Self::Slot => "Slot word",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Word {
    pub kind: WordKind,
    pub variant: String,
    pub lexeme: String,
    pub slot: String,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lexicon {
    words: Vec<Word>,
}

impl Lexicon {
    pub fn parse(source: &str) -> Result<Self, LexiconError> {
        let mut words = Vec::new();
        for (index, raw_line) in source.lines().enumerate() {
            let line = index + 1;
            let text = raw_line.trim();
            if text.is_empty() || text.starts_with('#') {
                continue;
            }

            let fields: Vec<&str> = text.split_whitespace().collect();
            if fields.len() != 4 {
                return Err(LexiconError::InvalidFieldCount {
                    line,
                    got: fields.len(),
                });
            }

            let Some(kind) = WordKind::parse(fields[0]) else {
                return Err(LexiconError::UnknownKind {
                    line,
                    value: fields[0].to_owned(),
                });
            };
            let lexeme = fields[1].to_owned();
            let variant = fields[2].to_owned();
            let slot = fields[3].to_owned();
            validate_rust_identifier(&variant).map_err(|reason| {
                LexiconError::InvalidIdentifier {
                    line,
                    value: variant.clone(),
                    reason,
                }
            })?;
            validate_lexeme(&lexeme).map_err(|reason| LexiconError::InvalidLexeme {
                line,
                value: lexeme.clone(),
                reason,
            })?;
            validate_grammar_slot(&slot).map_err(|reason| LexiconError::InvalidSlot {
                line,
                value: slot.clone(),
                reason,
            })?;

            words.push(Word {
                kind,
                variant,
                lexeme,
                slot,
                line,
            });
        }

        let lexicon = Self { words };
        lexicon.validate_unique()?;
        lexicon.validate_counts()?;
        Ok(lexicon)
    }

    #[must_use]
    pub fn words(&self) -> &[Word] {
        &self.words
    }

    pub fn words_of(&self, kind: WordKind) -> impl Iterator<Item = &Word> {
        self.words.iter().filter(move |word| word.kind == kind)
    }

    #[must_use]
    pub fn count(&self, kind: WordKind) -> usize {
        self.words_of(kind).count()
    }

    fn validate_counts(&self) -> Result<(), LexiconError> {
        let actual = [
            self.count(WordKind::Hard),
            self.count(WordKind::Modifier),
            self.count(WordKind::Slot),
        ];
        let expected = [HARD_KEYWORD_COUNT, MODIFIER_WORD_COUNT, SLOT_WORD_COUNT];
        if actual != expected {
            return Err(LexiconError::CountMismatch { actual, expected });
        }
        Ok(())
    }

    fn validate_unique(&self) -> Result<(), LexiconError> {
        for (index, left) in self.words.iter().enumerate() {
            for right in &self.words[index + 1..] {
                if left.lexeme == right.lexeme {
                    return Err(LexiconError::DuplicateLexeme {
                        first_line: left.line,
                        second_line: right.line,
                        value: left.lexeme.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexiconError {
    InvalidFieldCount {
        line: usize,
        got: usize,
    },
    UnknownKind {
        line: usize,
        value: String,
    },
    InvalidIdentifier {
        line: usize,
        value: String,
        reason: &'static str,
    },
    InvalidLexeme {
        line: usize,
        value: String,
        reason: &'static str,
    },
    InvalidSlot {
        line: usize,
        value: String,
        reason: &'static str,
    },
    DuplicateLexeme {
        first_line: usize,
        second_line: usize,
        value: String,
    },
    CountMismatch {
        actual: [usize; 3],
        expected: [usize; 3],
    },
}

impl fmt::Display for LexiconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFieldCount { line, got } => {
                write!(f, "line {line}: expected 4 fields, got {got}")
            }
            Self::UnknownKind { line, value } => {
                write!(f, "line {line}: unknown word kind {value:?}")
            }
            Self::InvalidIdentifier {
                line,
                value,
                reason,
            } => {
                write!(f, "line {line}: invalid Rust variant {value:?}: {reason}")
            }
            Self::InvalidLexeme {
                line,
                value,
                reason,
            } => {
                write!(f, "line {line}: invalid lexeme {value:?}: {reason}")
            }
            Self::InvalidSlot {
                line,
                value,
                reason,
            } => {
                write!(f, "line {line}: invalid slot {value:?}: {reason}")
            }
            Self::DuplicateLexeme {
                first_line,
                second_line,
                value,
            } => write!(
                f,
                "duplicate lexeme {value:?} on lines {first_line} and {second_line}"
            ),
            Self::CountMismatch { actual, expected } => write!(
                f,
                "word counts are hard={} modifier={} slot={}, expected hard={} modifier={} slot={}",
                actual[0], actual[1], actual[2], expected[0], expected[1], expected[2]
            ),
        }
    }
}

impl std::error::Error for LexiconError {}

fn validate_rust_identifier(value: &str) -> Result<(), &'static str> {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return Err("must start with an ASCII letter or underscore"),
    }
    if chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        Ok(())
    } else {
        Err("must contain only ASCII letters, digits, and underscores")
    }
}

fn validate_lexeme(value: &str) -> Result<(), &'static str> {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return Err("must start with an ASCII letter or underscore"),
    }
    if chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_') {
        Ok(())
    } else {
        Err("must contain only ASCII letters, digits, and underscores")
    }
}

fn validate_grammar_slot(value: &str) -> Result<(), &'static str> {
    if value.is_empty() {
        return Err("must not be empty");
    }
    if value.split('_').all(|part| {
        part.chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
    }) {
        Ok(())
    } else {
        Err("must contain only lowercase snake_case words")
    }
}

fn rust_enum(kind: WordKind) -> &'static str {
    match kind {
        WordKind::Hard => "HardKeyword",
        WordKind::Modifier => "ModifierWord",
        WordKind::Slot => "SlotWord",
    }
}

#[must_use]
pub fn generate_rust(lexicon: &Lexicon) -> String {
    let mut out = String::new();
    out.push_str("// @generated by syntaxgen from docs/syntax/lexicon.txt.\n");
    out.push_str(
        "// Do not edit this file; update the lexicon and run `cargo run -p syntaxgen`.\n\n",
    );

    for kind in [WordKind::Hard, WordKind::Modifier, WordKind::Slot] {
        out.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n");
        out.push_str(&format!("pub enum {} {{\n", rust_enum(kind)));
        for word in lexicon.words_of(kind) {
            out.push_str(&format!("    {},\n", word.variant));
        }
        out.push_str("}\n\n");

        out.push_str(&format!("impl {} {{\n", rust_enum(kind)));
        out.push_str("    #[must_use]\n");
        out.push_str("    pub fn as_str(self) -> &'static str {\n");
        out.push_str("        match self {\n");
        for word in lexicon.words_of(kind) {
            out.push_str(&format!(
                "            Self::{} => \"{}\",\n",
                word.variant, word.lexeme
            ));
        }
        out.push_str("        }\n    }\n}\n\n");

        let lookup_name = match kind {
            WordKind::Hard => "lookup_hard_keyword",
            WordKind::Modifier => "lookup_modifier_word",
            WordKind::Slot => "lookup_slot_word",
        };
        out.push_str("#[must_use]\n");
        out.push_str(&format!(
            "pub fn {lookup_name}(word: &str) -> Option<{}> {{\n",
            rust_enum(kind)
        ));
        out.push_str("    match word {\n");
        for entry in lexicon.words_of(kind) {
            out.push_str(&format!(
                "        \"{}\" => Some({}::{}),\n",
                entry.lexeme,
                rust_enum(kind),
                entry.variant
            ));
        }
        out.push_str("        _ => None,\n    }\n}\n\n");
    }

    out.push_str(&format!(
        "pub const HARD_KEYWORD_COUNT: usize = {HARD_KEYWORD_COUNT};\n"
    ));
    out.push_str(&format!(
        "pub const MODIFIER_WORD_COUNT: usize = {MODIFIER_WORD_COUNT};\n"
    ));
    out.push_str(&format!(
        "pub const SLOT_WORD_COUNT: usize = {SLOT_WORD_COUNT};\n"
    ));
    out
}

#[must_use]
pub fn generate_antlr(lexicon: &Lexicon) -> String {
    let mut out = String::new();
    out.push_str("// @generated by syntaxgen from docs/syntax/lexicon.txt.\n");
    out.push_str(
        "// Only HARD keywords are lexer tokens. Modifier and slot words stay IDENTIFIER\n",
    );
    out.push_str(
        "// and must be recognized by parser semantic predicates in their grammar slots.\n",
    );
    out.push_str("lexer grammar StarryLexer;\n\n");

    for word in lexicon.words_of(WordKind::Hard) {
        out.push_str(&format!("{} : '{}';\n", word.variant, word.lexeme));
    }
    out.push_str("\nIDENTIFIER : [A-Za-z_][A-Za-z0-9_]*;\n");
    out.push_str("\n// Contextual classifications (parser semantic predicates):\n");

    for kind in [WordKind::Modifier, WordKind::Slot] {
        let label = if kind == WordKind::Modifier {
            "modifier"
        } else {
            "slot"
        };
        for word in lexicon.words_of(kind) {
            out.push_str(&format!(
                "// {label}: {} = '{}' in {}\n",
                word.variant, word.lexeme, word.slot
            ));
        }
    }
    out
}

#[must_use]
pub fn generate_markdown(lexicon: &Lexicon) -> String {
    let mut out = String::new();
    out.push_str("# Starry v0.3 lexical vocabulary\n\n");
    out.push_str("@generated by `syntaxgen`; do not edit this file directly.\n\n");

    for kind in [WordKind::Hard, WordKind::Modifier, WordKind::Slot] {
        out.push_str(&format!(
            "## {} ({})\n\n",
            kind.title(),
            lexicon.count(kind)
        ));
        out.push_str("| Variant | Lexeme | Grammar slot |\n|---|---|---|\n");
        for word in lexicon.words_of(kind) {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` |\n",
                word.variant, word.lexeme, word.slot
            ));
        }
        out.push('\n');
    }
    out
}

#[must_use]
pub fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("syntaxgen is stored at <workspace>/crates/syntaxgen")
}

#[must_use]
pub fn paths(root: &Path) -> [PathBuf; 3] {
    [
        root.join(KEYWORD_GEN_PATH),
        root.join(ANTLR_TERMS_PATH),
        root.join(MARKDOWN_TABLE_PATH),
    ]
}

pub fn contents(root: &Path) -> Result<Vec<String>, std::io::Error> {
    let source = fs::read_to_string(root.join(LEXICON_PATH))?;
    let lexicon = Lexicon::parse(&source)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    Ok(vec![
        generate_rust(&lexicon),
        generate_antlr(&lexicon),
        generate_markdown(&lexicon),
    ])
}

pub fn write_all(root: &Path) -> Result<(), std::io::Error> {
    let generated = contents(root)?;
    for (path, text) in paths(root).into_iter().zip(generated) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, text)?;
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckError {
    Io(String),
    Stale(PathBuf),
    Lexicon(String),
}

impl fmt::Display for CheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) => write!(f, "cannot read generated file: {message}"),
            Self::Stale(path) => write!(f, "generated file is stale: {}", path.display()),
            Self::Lexicon(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CheckError {}

pub fn check(root: &Path) -> Result<(), CheckError> {
    let generated = contents(root).map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            CheckError::Lexicon(error.to_string())
        } else {
            CheckError::Io(error.to_string())
        }
    })?;

    for (path, expected) in paths(root).into_iter().zip(generated) {
        let actual = fs::read_to_string(&path)
            .map_err(|error| CheckError::Io(format!("{}: {error}", path.display())))?;
        if actual != expected {
            return Err(CheckError::Stale(path));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lexicon() -> Lexicon {
        let source = fs::read_to_string(workspace_root().join(LEXICON_PATH)).unwrap();
        Lexicon::parse(&source).unwrap()
    }

    #[test]
    fn parses_frozen_counts() {
        let words = lexicon();
        assert_eq!(words.count(WordKind::Hard), HARD_KEYWORD_COUNT);
        assert_eq!(words.count(WordKind::Modifier), MODIFIER_WORD_COUNT);
        assert_eq!(words.count(WordKind::Slot), SLOT_WORD_COUNT);
    }

    #[test]
    fn rejects_duplicate_lexeme_before_count_validation() {
        let source = "hard x X global\nhard x X global\n";
        assert_eq!(
            Lexicon::parse(source),
            Err(LexiconError::DuplicateLexeme {
                first_line: 1,
                second_line: 2,
                value: "x".to_owned(),
            })
        );
    }

    #[test]
    fn generates_expected_shapes() {
        let words = lexicon();
        let rust = generate_rust(&words);
        assert!(rust.contains("pub enum HardKeyword {"));
        assert!(rust.contains("pub enum ModifierWord {"));
        assert!(rust.contains("pub enum SlotWord {"));
        assert!(rust.contains("\"if\" => Some(HardKeyword::If),"));

        let antlr = generate_antlr(&words);
        assert!(antlr.contains("If : 'if';"));
        assert!(antlr.contains("IDENTIFIER : [A-Za-z_][A-Za-z0-9_]*;"));
        assert!(antlr.contains("// modifier: Public = 'public' in modifier"));
        assert!(antlr.contains("// slot: Before = 'before' in aspect_advice"));

        let markdown = generate_markdown(&words);
        assert!(markdown.contains("## Hard keyword (39)"));
        assert!(markdown.contains("## Modifier word (14)"));
        assert!(markdown.contains("## Slot word (31)"));
    }

    #[test]
    fn generated_files_are_current() {
        check(workspace_root()).unwrap();
    }
}
