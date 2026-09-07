# Starry

A textbook compiler project written in pure Rust std (zero external
dependencies). Every phase of the classic compiler pipeline is
hand-written from its mathematical foundations — the goal is to
implement the theory through code, not to minimize code.

## Pipeline

Implemented today (regex → NFA):

```text
regex string
  | parse      regex::parse           -> Ast
  | translate  regex::Translate       -> Hir   normalization, (?i) scoping, repeat unfolding
  | optimize   inside NFA::from_hir   -> Hir   fixpoint-equivalent rewrites (single entry)
  | compile    nfa::Builder::compile  -> NFA   Thompson construction
  | simulate   NFA::match_prefix      -> Option<usize>  longest match
```

## CLI

```bash
starryc match "[a-zA-Z][a-zA-Z0-9_]*" "abc123"   # stdout: 6
starryc dump-nfa "a|b"                            # prints the NFA
```

Exit codes: `0` match (length on stdout) · `1` no match (stderr
`no match`) · `2` usage error.

## Project Structure

```
starry/
├── crates/
│   ├── ast/       # Token / Keyword / TokenKind definitions (pure std)
│   ├── lex/       # regex -> NFA pipeline + automaton state layer
│   └── starryc/   # compiler executable (currently the regex pipeline CLI)
└── Cargo.toml     # workspace
```

## Roadmap (textbook order)

- [x] Subset-construction DFA; Hopcroft / Moore / Brzozowski
  minimization; lexer driver (longest match, keyword post-
  classification, error recovery); regular grammar <-> NFA.
- Byte equivalence classes (char vs UTF-8 byte intervals) and an ASCII
  direct table (`[Option<StateId>; 128]`).
- Recursive-descent / LR parser and the language-level AST for Starry.
- Semantic analysis: symbol tables, scopes, type checking.
- Intermediate representation (three-address code / SSA) and
  optimization passes (constant folding, copy propagation, DCE).
- LLVM IR text emission (pure std string building), then
  llvm-as/llc/clang for multi-platform executables.
- Builder state budget (make from_hir fallible to guard against
  nested repeat products); `(?i:...)` scoped groups.
- NFA storage: CSR + bitset with concrete-typed hot loops;
  zero-copy token spans + interning.

## Running Tests

```bash
cargo test
```

## Requirements

- Rust 2024 Edition
- Rust 1.85.0 or later

## License

Licensed under the Apache-2.0. See [LICENSE](LICENSE) for details.
