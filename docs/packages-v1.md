# Path-based package graph v1

## Manifest

`starry.toml` uses a strict, hand-parsed TOML subset. No other keys or tables are accepted.

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
- String escapes, unknown keys, duplicate tables, duplicate keys, trailing commas,
  and keys outside a known table are rejected with `file:line:col` diagnostics.
- A directory without `starry.toml` remains an implicit single-root build for
  compatibility. A dependency target must have `starry.toml`.

## Graph

- Package identity is the package name. Canonicalized roots distinguish identity:
  the same name from two different canonical roots is a duplicate-package error.
- The graph stores package manifests, dependency specs, canonical resolved roots,
  and dependency edges in memory.
- Dependency traversal is deterministic and dependency-first. Ordered containers
  make source-tree read order irrelevant.
  Source roots are traversed without following symlinks. Source files are collected per package in graph postorder and then sorted by
  path. v1 still lowers those files into one merged source set, but every merged
  unit records its owning `PackageId`, and that ownership is checked against the
  package's canonical root.
- Import resolution is conceptually `PackageId + ModulePath + Symbol`.

## Imports, visibility, and cycles

- Imports are explicit and non-transitive: a package can import only its own
  sources and its direct dependencies.
- Cross-package imports can use only `public` symbols. `internal` remains
  package-local. `private` and `protected` follow the existing source-level rules.
- Dependency cycles are rejected with the complete deterministic path, for
  example `dependency cycle: a -> b -> a`.
- Import cycles and self-imports are rejected, for example
  `import cycle: a -> b -> a` or `import cycle: app -> app`.
- Single-file builds without a manifest continue to work.
- Module paths are compiler metadata, never source syntax: manifest packages
  prefix the manifest name, then the directory path relative to `src_root`
  (`math/lib.starry` -> `math`, `math/src/vec/util.starry` -> `math.vec`);
  manifest-less roots use the bare directory path (`demo/core/util.starry` ->
  `demo.core`). One directory is one module; directory and package names must
  be valid dotted identifiers.

## Non-goals in v1

- Registry resolution, lockfiles, aliases, feature flags, and workspace discovery.
- Version ranges (`>=`, carets), prerelease resolution, or version negotiation.
- Dynamic linking or public ABI mangling.
- Separate artifact caching per package or incremental graph rebuilds.
