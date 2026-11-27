+++
title = "CLI"
description = "Current Starry command-line interface."
+++

# CLI

## Commands

| Command | Purpose |
|---|---|
| `starryc match <regex> <input>` | Run the regular-expression probe and show the longest match length. |
| `starryc dump-nfa <regex>` | Print NFA states and transitions. |
| `starryc tokenize <source>` | Print the lexical token stream. |
| `starryc check <file.starry>` | Parse a file and run semantic checks. |
| `starryc irc <file.starry>` | Print three-address code. |
| `starryc opt <file.starry>` | Print optimized three-address code. |
| `starryc llvme <file.starry>` | Print LLVM IR text. |
| `starryc build <file.starry> [out]` | Compile and link a native executable. |
| `starryc build <dir|starry.toml> [out]` | Build a package from a manifest or directory. |

## Build arguments

After `--`, `starryc build` passes linker arguments to the native toolchain. For `extern "c"` calls, these arguments may contain object files, library names, and library paths.

```bash
starryc build program.starry program -- math.obj -lm
```

## Exit codes

`check`, `irc`, `opt`, `llvme`, and `build` return `0` when the input is accepted and `1` when it produces a source diagnostic.

Diagnostics use this form:

```text
file.starry:line:column: Code: message
```
