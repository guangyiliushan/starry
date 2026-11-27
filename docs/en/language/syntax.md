+++
title = "Syntax"
description = "Current Starry declarations, expressions, and lexical forms."
+++

# Syntax

## Declarations

`val` creates an immutable binding. `var` creates a mutable binding. A function declares its parameters and return type explicitly when it returns a value.

```starry run
fun add(left: int, right: int): int {
    return left + right
}

fun main(): int {
    val fixed = 10
    var changeable = 20
    changeable = 21
    print(fixed)
    print(changeable)
    print(add(4, 5))
    return 0
}
```

```text
10
21
9
```

## Literals

Integers accept decimal, hexadecimal, octal, and binary forms. Floating-point literals require a fraction or exponent. Character literals are Unicode. Strings interpolate identifiers and `${}` expressions.

```starry run
fun describe(count: int): string {
    return "count=${count}"
}

fun main(): int {
    val decimal = 42
    val hexadecimal = 0xff
    val octal = 0o17
    val binary = 0b101
    val real = 2.5
    val letter = 'A'
    val text = "value ${decimal}"
    print(decimal)
    print(hexadecimal)
    print(octal)
    print(binary)
    print(real)
    print(letter)
    print(text)
    print(describe(3))
    return 0
}
```

```text
42
255
15
5
2.5
A
value 42
count=3
```

## Expressions

Expressions support arithmetic, comparison, logical short-circuiting, negation, and function calls.

```starry check
fun main(): int {
    val sum = 7 + 2
    val product = 7 * 2
    val ordered = 1 < 2 && 2 < 3
    val alternative = false || true
    val inverted = !false
    print(sum)
    print(product)
    print(ordered)
    print(alternative)
    print(inverted)
    return 0
}
```

## Statements

Newlines end statements. A line continues when it ends with a binary operator or the next line starts with a binary-only operator or `.`.

```starry run
fun main(): int {
    val total = 1 +
        2
    print(total)
    return 0
}
```

```text
3
```
