+++
title = "Functions and closures"
description = "Current Starry functions, arguments, function types, and closures."
+++

# Functions and closures

## Functions

Functions can be declared at the top level, in classes, and in traits. A call may use positional arguments, named arguments, or defaults.

```starry run
fun greet(name: string = "Starry", punctuation: string = "!"): string {
    return "hello ${name}${punctuation}"
}

fun main(): int {
    print(greet())
    print(greet("Ada"))
    print(greet(name = "Grace", punctuation = "?"))
    return 0
}
```

```text
hello Starry!
hello Ada!
hello Grace?
```

## Function types

A function type lists parameter types and its return type, such as `(int) -> int`.

```starry run
fun twice(value: int): int {
    return value * 2
}

fun apply(operation: (int) -> int, value: int): int {
    return operation(value)
}

fun main(): int {
    print(apply(twice, 8))
    return 0
}
```

```text
16
```

## Closures

A closure is written in braces. Parameters precede `->`; a single unparameterized closure can use `it`.

```starry run
fun apply(operation: (int) -> int, value: int): int {
    return operation(value)
}

fun main(): int {
    print(apply({ n -> n * 3 }, 4))
    print(apply({ (number: int) -> number + 1 }, 4))
    return 0
}
```

```text
12
5
```

## Captured values

A closure captures the values it uses. Captured values remain usable inside the closure body.

```starry run
fun makeCounter(start: int): (int) -> int {
    return { (amount: int) -> start + amount }
}

fun main(): int {
    val advance = makeCounter(10)
    print(advance(2))
    print(advance(5))
    return 0
}
```

```text
12
15
```
