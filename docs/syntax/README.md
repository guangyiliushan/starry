# Starry lexical source of truth

`lexicon.txt` is the single source for Starry v0.3 vocabulary. `syntaxgen`
generates the Rust definitions, the reserved-token fragment for the ANTLR4
lexer reference, and the Markdown table.

The frozen counts are 35 hard keywords, 10 modifier words, and 8 slot words (53 total; frozen 2026-11-25, amended the same day when `static` was removed, see keyword-capability-plan).
Only hard keywords are reserved by the lexer. Modifier and slot words remain
identifiers and are recognized by parser predicates only in their listed
grammar slots.

## Frozen syntax decisions

`let` is not a Starry keyword and is not a declaration form. Simple refutable
conditions use `is`:

```starry
if value is Some(v) {
    use(v)
}
```

For complex destructuring conditions, reuse the existing `val` binding form in
the condition slot:

```starry
if val Some(Response { status, body: Json(data) }) = response {
    accept(data)
}

while val Some((key, value)) = queue.next() {
    handle(key, value)
}
```

The `val` in this condition slot is a refutable binding: the branch runs only
when the pattern matches. There is no `let` declaration and no `let` condition.


```bash
cargo run -p syntaxgen
cargo run -p syntaxgen -- --check
```

Generated files:

- `crates/ast/src/token/keyword.rs`
- `docs/syntax/generated/StarryLexerTerms.g4`
- `docs/syntax/generated/lexicon.md`

Do not edit generated files. Change `lexicon.txt`, regenerate, and commit all
products together.
