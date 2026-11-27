+++
title = "Control flow and patterns"
description = "Current Starry branching, loops, labels, defer, and patterns."
+++

# Control flow and patterns

## Branching

`if` branches on a Boolean. `when` compares one subject against patterns and must be exhaustive when its value is used.

```starry run
fun classify(value: int): string {
    return when value {
        0 -> "zero",
        1 -> "one",
        else -> "many",
    }
}

fun main(): int {
    if 1 < 2 {
        print("ordered")
    }
    print(classify(0))
    print(classify(1))
    print(classify(9))
    return 0
}
```

```text
ordered
zero
one
many
```

## Patterns

Patterns bind values from constructors, arrays, tuples, and structs. `is` tests one pattern in a condition; `when` tests alternatives.

```starry run
enum Value {
    Number(int),
    Text(string),
}

fun main(): int {
    val value = Value.Number(4)
    if value is Value.Number(n) {
        print(n)
    }
    return 0
}
```

```text
4
```

## Loops

`for` iterates arrays and ranges. `while` repeats while a condition holds. `loop` repeats until a `break`.

```starry run
fun main(): int {
    for value in 1..=3 {
        if value == 2 {
            continue
        }
        print(value)
    }

    var count = 2
    while count > 0 {
        print(count)
        count = count - 1
    }

    loop {
        print("once")
        break
    }
    return 0
}
```

```text
1
3
2
1
once
```

## Labels and values

A label names a loop. `break LABEL VALUE` exits that loop with a value.

```starry run
fun pick(): int {
    'outer: loop {
        break 'outer 42
    }
}

fun main(): int {
    print(pick())
    return 0
}
```

```text
42
```

## Deferred execution

`defer` runs at the end of the enclosing scope, in reverse order.

```starry run
fun work(): int {
    defer {
        print("cleanup")
    }
    print("work")
    return 0
}

fun main(): int {
    return work()
}
```

```text
work
cleanup
```
