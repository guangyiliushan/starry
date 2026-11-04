# Starry

A textbook compiler project written in pure Rust std (zero external
dependencies). The repository implements the classic compiler pipeline by
hand: lexer generator, recursive-descent parser, semantic analysis,
three-address intermediate code, optimization passes, and LLVM IR text
emission.

## Pipeline

```text
SourceFile
  -> lex      lexer tables + longest-match driver
  -> parser   recursive descent + Pratt expressions + patterns
  -> sem      scopes, defs, types, diagnostics, flow resolution
  -> irc      three-address code + reference interpreter
  -> opt      constant folding, copy propagation, DCE, local CSE, LICM
  -> llvme    typed LLVM IR text (opaque pointers, i64 slot layout)
```

`starryc llvme` emits `.ll` text. If `llvm-as` or `clang` is available, the
integration tests use it as an external validator; neither tool is a crate
dependency.

## CLI

```bash
starryc match <regex> <input>       # regex engine probe: longest match length
starryc dump-nfa <regex>            # print NFA states and transitions
starryc tokenize <source>           # print lexical token stream
starryc check <file.starry>         # parse + semantic diagnostics
starryc irc <file.starry>           # print TAC
starryc opt <file.starry>           # print optimized TAC
starryc llvme <file.starry>         # print LLVM IR
starryc build <file.starry> [out]  # compile and link a native executable
```

`check`, `irc`, `opt`, `llvme`, and `build` exit `0` when clean and `1` on source
diagnostics. Machine failures such as unreadable files also exit `1`.

## Project structure

```text
crates/
  ast/       shared spans, interner, AST nodes, diagnostics
  lex/       regex -> NFA -> DFA lexer machinery and token definitions
  parser/    recursive-descent parser and syntax snapshots
  sem/       declaration, name resolution, type checking, move checks
  irc/       TAC generation, layout, and reference interpreter
  opt/       textbook optimization passes on TAC
  llvme/     LLVM IR text emission
  starryc/   command-line compiler driver
  syntaxgen/ generated syntax vocabulary checks
runtime/
  starry_runtime.c  # freestanding C runtime used only by starryc build
```

Grammar and phase contracts live in `docs/syntax/`: `starry.ebnf`,
`lexicon.txt`, `parser-notes.md`, `semantic-notes.md`, `coverage.md`, and
`opt-notes.md`.

## Status

The implemented surface covers scalar and aggregate data, control flow,
patterns, nullable values, string interpolation, print I/O, calls with defaults, and the
optimization pipeline listed above. See `semantic-notes.md` and
`opt-notes.md` for the intentionally deferred language features.

## Running tests

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p syntaxgen -- --check
```

## Requirements

- Rust 2024 Edition
- Rust 1.85.0 or later

## License

Licensed under the Apache-2.0. See [LICENSE](LICENSE) for details.
