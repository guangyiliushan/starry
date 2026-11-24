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
  -> llvme    typed LLVM IR text (opaque pointers, type-directed slots)
```

`starryc llvme` emits `.ll` text and, when `clang` is available, validates it
before printing (invalid IR exits `1`). If `llvm-as` or `clang` is available,
the integration tests use it as an external validator; neither tool is a crate
Toolchain baseline: LLVM 15+ (opaque pointers; the IR verifier runs `llvm-as`
first, falling back to `clang -x ir -c`). Tests that need the toolchain print an
explicit `SKIP:` when it is absent — never silent. Native builds use the
freestanding Windows runtime (`mainCRTStartup` + kernel32) or the normal libc
startup on Linux; both use the same stable Starry runtime symbols and trap exit
code 134.
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
starryc build <file.starry> [out] [-- <clang link args>]  # compile and link a native executable (extern "c" FFI: extra .obj / -l / -L after --)
starryc build <dir|starry.toml> [out]  # multi-file: manifest = package identity; module path = name + directory
```

`check`, `irc`, `opt`, `llvme`, and `build` exit `0` when clean and `1` on source
diagnostics. Diagnostics are printed as `path:line:col: Code: message`.

## Sample corpus

- `samples/*.starry` — runnable examples; every file must pass `starryc check`, and (with a clang toolchain) `starryc build` + run must reproduce its golden output in `samples/golden/<name>.stdout`.
- `samples/negative/*.starry` — syntax/semantics the language rejects on purpose.
- `samples/roadmap/*.starry` — spec examples for features not implemented yet
  (traits, classes, impl blocks, spawn/actor, generics).

The `samples_cli` test enforces all of this: check-gate, negative-gate,
roadmap-gate, and the run-pass with golden stdout.

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
  common.c          # shared Starry runtime logic and stable symbols
  win32.c           # Windows freestanding: Heap*, WriteFile, ExitProcess
  posix.c           # Linux libc: malloc/free, write, _exit
```

Grammar and phase contracts live in `docs/syntax/`: `starry.ebnf`,
`lexicon.txt`, `parser-notes.md`, `semantic-notes.md`, `coverage.md`, and
`opt-notes.md`.

## Status

The implemented surface covers scalar and aggregate data, control flow,
patterns, nullable values, string interpolation and concatenation, print I/O,
`extern "c"` calls, structural equality, defaults, top-level globals initialized
in dependency order, class/ARC (constructor, methods, deinit, `weak var`,
nullable classes), generics (monomorphization), traits with static dispatch,
closures with value capture, directory/manifest module paths, and the
optimization pipeline listed above. Intentionally rejected surfaces (stable
diagnostics) are listed in `docs/syntax/feature-matrix.md`. See
`semantic-notes.md` and `opt-notes.md` for the intentionally deferred language
features.

## Running tests

### LLVM toolchain discovery

The LLVM validator and `starryc build` discover tools in this order:

1. `llvm-as` / `clang` on `PATH`.
2. `C:\Program Files\LLVM\bin`.
3. `C:\Program Files (x86)\LLVM\bin`.

If none is found, validator tests print an explicit `SKIP:` line and the
native-build end-to-end test skips; they do not fail the suite. To force a
local LLVM install on Windows:

```powershell
$env:PATH = "C:\Program Files\LLVM\bin;" + $env:PATH
cargo test --workspace --target-dir target\codex-check
```

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
