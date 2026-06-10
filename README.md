# Starry

A modular compiler infrastructure written in Rust.

## Overview

Starry is a compiler infrastructure project currently focused on implementing fundamental algorithms and concepts from compiler theory, including lexical analysis, syntax parsing, and automata theory. The project serves as both an educational platform for understanding compiler construction and a foundation for future language development.

The long-term vision for Starry is to evolve into a modern multi-paradigm programming language that maintains full C/C++ compatibility while incorporating Kotlin-style syntactic sugar. The planned language will feature a hybrid type system combining static and dynamic typing, region-based memory management with borrow checking, and support for multiple programming paradigms including object-oriented, functional, and reactive programming.

## Project Structure

```
starry/
├── crates/
│   ├── starry-lex/      # Lexer library with regex and automata support
│   ├── starry-parser/   # Parser library with LL(1) and LR family support
│   ├── starry-ast/      # Abstract Syntax Tree definitions
│   ├── starry-semantic/ # Semantic analysis pipeline and IR generation
│   ├── starry/          # Core library integration (stub)
│   └── starryc/         # Compiler executable (stub)
└── Cargo.toml           # Workspace configuration
```

## Running Tests

```bash
cargo test
```

## Current Features

The current implementation features a fully functional lexer with regular expression support, NFA/DFA construction, and DFA minimization algorithms; a mature parser supporting LL(1) (table-driven and recursive descent) and the full LR family (LR(0), SLR(1), LR(1), LALR(1)) with CFG analysis (FIRST/FOLLOW sets, left recursion detection and elimination, phrase analysis); a complete AST library with span tracking and token types; and a multi-pass semantic analysis pipeline covering name resolution, type checking, and IR generation (TAC/quadruples/triples, basic blocks, DAG), with rule-driven attribute evaluation.

## Requirements

- Rust 2024 Edition
- Rust 1.85.0 or later

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
