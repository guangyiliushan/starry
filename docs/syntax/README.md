# Starry lexical source of truth

`lexicon.txt` is the single source for Starry v0.3 vocabulary. `syntaxgen`
generates the Rust definitions, the reserved-token fragment for the ANTLR4
lexer reference, and the Markdown table.

The frozen counts are 39 hard keywords, 14 modifier words, and 31 slot words.
Only hard keywords are reserved by the lexer. Modifier and slot words remain
identifiers and are recognized by parser predicates only in their listed
grammar slots.

```bash
cargo run -p syntaxgen
cargo run -p syntaxgen -- --check
```

Generated files:

- `crates/ast/src/token/keyword_gen.rs`
- `docs/syntax/generated/StarryLexerTerms.g4`
- `docs/syntax/generated/lexicon.md`

Do not edit generated files. Change `lexicon.txt`, regenerate, and commit all
products together.
