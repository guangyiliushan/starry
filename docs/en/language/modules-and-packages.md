+++
title = "Modules and packages"
description = "Current Starry imports, visibility, module paths, and package rules."
+++

# Modules and packages

Starry has no per-file package header. The compiler derives a module path from the package name and source-file path.

## Imports

Imports are explicit and appear at the top of a source file.

```starry syntax
import util.helper
import geometry.shapes.*
import json.reader as reader
```

`name` imports one symbol, `name.*` imports a module namespace, and `name as alias` creates a local alias.

## Visibility

Declarations can be restricted to their package or module.

```starry check
public fun visible(): int {
    return 1
}

fun main(): int {
    print(visible())
    return 0
}
```

```text
1
```

## Module paths

One directory is one module. In a package, the manifest name starts the path. Directory and package names use dotted identifiers.

```text
math/lib.starry       -> math
math/src/vec/util.starry -> math.vec
demo/core/util.starry -> demo.core
```

## Package boundaries

A package can import its own sources and its direct dependencies only. Cross-package imports can use `public` symbols. Dependency and import cycles are rejected.
