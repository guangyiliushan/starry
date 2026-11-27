+++
title = "Packages"
description = "Current Starry package manifests, dependencies, visibility, and cycles."
+++

# Packages

A package is identified by `starry.toml`. A directory without a manifest remains an implicit single-root build.

## Manifest

```toml
[package]
name = "app"
version = "1.0.0"

[dependencies]
math = { path = "../math", version = "1.2.0" }
```

Rules:

- `[package].name` and `[package].version` are required non-empty strings.
- `[dependencies]` is optional. Bare dependency keys are package names.
- Each dependency inline table contains exactly `path` and `version`.
- `path` resolves relative to the depending package's canonical root.
- `version` is an exact string match against the target `[package].version`.
- The dependency key must equal the target package's `package.name`.
- String escapes, unknown keys, duplicate tables, duplicate keys, trailing commas, and keys outside a known table are rejected with `file:line:col` diagnostics.

## Package graph

Package identity is the package name. Canonicalized roots distinguish identity, so the same name from two different roots is a duplicate-package error.

The graph stores manifests, dependency specifications, canonical roots, and dependency edges. Traversal is deterministic and dependency-first. Source files are collected in graph postorder and then sorted by path.

## Imports and visibility

Imports are explicit and non-transitive. A package can import its own sources and direct dependencies only. Cross-package imports can use `public` symbols. `internal` remains package-local. `private` and `protected` follow source-level rules.

Dependency cycles are rejected with a complete deterministic path, such as `dependency cycle: a -> b -> a`. Import cycles and self-imports are rejected the same way.

## Module paths

Module paths are compiler metadata, not source syntax. In a package, the manifest name starts the path, followed by the directory path relative to the source root. A directory is one module. Directory and package names use dotted identifiers.

```text
math/lib.starry          -> math
math/src/vec/util.starry -> math.vec
demo/core/util.starry    -> demo.core
```
