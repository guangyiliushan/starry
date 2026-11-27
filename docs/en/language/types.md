+++
title = "Types"
description = "Current Starry scalar, nullable, aggregate, and user-defined types."
+++

# Types

## Scalars

Starry provides integers, floating-point numbers, Booleans, characters, strings, and `unit`. Integer literals adapt to the expected width when one is known; otherwise they stay within `i64` or widen to `i128`.

```starry run
fun main(): int {
    val whole: int = 7
    val real: f64 = 2.5
    val flag: bool = true
    val letter: char = 'A'
    val text: string = "Starry"
    print(whole)
    print(real)
    print(flag)
    print(letter)
    print(text)
    return 0
}
```

```text
7
2.5
true
A
Starry
```

## Nullable values

A trailing `?` makes a nullable type. `null` creates the absent value. `??` supplies a replacement, and `!!` unwraps a present value.

```starry run
fun main(): int {
    val present: int? = 41
    val absent: int? = null
    print(present!! + 1)
    print(absent ?? 0)
    return 0
}
```

```text
42
0
```

## Arrays and ranges

Array literals use square brackets. Indexing uses square brackets. A range can be exclusive with `..` or inclusive with `..=`.

```starry run
fun main(): int {
    val values = [10, 20, 30]
    print(values[1])
    for value in 1..=3 {
        print(value)
    }
    return 0
}
```

```text
20
1
2
3
```

## Structs

A struct declares named fields. Primary-constructor parameters marked `val` or `var` become fields; unmarked parameters are construction-time values.

```starry run
struct Point(val x: int, var y: int)

fun main(): int {
    val point = Point(1, 2)
    point.y = 3
    print(point.x + point.y)
    return 0
}
```

```text
4
```

## Enums

An enum has a closed set of variants. A variant may carry values.

```starry check
enum Shape {
    Circle(int),
    Square(int),
}

fun main(): int {
    val shape = Shape.Circle(2)
    print(shape is Shape.Circle(_))
    return 0
}
```

## Tuples

A tuple groups a fixed number of values. Tuples use constant integer indices.

```starry check
fun main(): int {
    val pair = (1, "one")
    print(pair[0])
    print(pair[1])
    return 0
}
```
