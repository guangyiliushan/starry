+++
title = "Diagnostics"
description = "Current stable Starry diagnostics with minimal examples."
+++

# Diagnostics

A diagnostic identifies the file, line, column, stable code, and message. The file path in the examples below is `program.starry`.

## Undefined name

```starry reject
fun main(): int {
    print(missing)
    return 0
}
```

```text
program.starry:2:11: SemUndefinedName: undefined name `missing`
```

## Assignment to `val`

```starry reject
fun main(): int {
    val value = 1
    value = 2
    return 0
}
```

```text
program.starry:3:5: SemAssignToVal: cannot assign to an immutable binding
```

## Missing argument

```starry reject
fun need(value: int): int {
    return value
}

fun main(): int {
    print(need())
    return 0
}
```

```text
program.starry:6:11: SemArityMismatch: missing argument for parameter `value`
```

## Non-exhaustive `when`

```starry reject
fun classify(value: int): int {
    return when value {
        0 -> 0
    }
}

fun main(): int {
    print(classify(0))
    return 0
}
```

```text
program.starry:2:12: SemMissingElse: when requires an else branch
```
