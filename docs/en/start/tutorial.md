+++
title = "Tutorial"
description = "Write and run a first Starry program."
+++

# Tutorial

This walkthrough builds `starryc`, writes complete programs, and checks their output.

Build the compiler:

```bash
cargo build -p starryc
```

## Values and interpolation

```starry run
fun main(): int {
    val name = "Starry"
    print("hello ${name}")
    if 1 + 1 == 2 {
        print("ok")
    }
    return 0
}
```

```text
hello Starry
ok
```

## Structs and arrays

```starry run
struct Point(val x: int, var y: int)

fun main(): int {
    val p = Point(1, 2)
    p.y = 7
    val points = [p, Point(3, 4)]
    print(points[0].x + points[0].y)
    print(points[1].x + points[1].y)
    return 0
}
```

```text
8
7
```

## Classes and release-time cleanup

```starry run
class Counter(var count: int) {
    fun bump(): int {
        self.count += 1
        return self.count
    }

    deinit {
        print(self.count)
    }
}

fun main(): int {
    val a = Counter(10)
    val b = a
    print(b.bump())
    return 0
}
```

```text
11
11
```

## Abstract classes

```starry check
abstract class Shape(val name: int) {
    abstract fun area(): int;
}

open class Square(val side: int): Shape(side) {
    override fun area(): int {
        return self.side * self.side
    }
}

fun main(): int {
    val shape: Shape = Square(4)
    print(shape.area())
    return 0
}
```

A concrete subclass must override every inherited abstract method. An abstract class cannot be instantiated.

## Traits and dynamic dispatch

```starry run
trait Shape {
    fun area(): int
}

class Square(val side: int) {
}

impl Shape for Square {
    fun area(): int {
        return self.side * self.side
    }
}

fun main(): int {
    val shape: dyn Shape = Square(4)
    print(shape.area())
    return 0
}
```

```text
16
```

A rejected program reports a diagnostic with a file, line, column, stable code, and message.

```starry reject
fun main(): int {
    print(missing)
    return 0
}
```

```text
program.starry:2:11: SemUndefinedName: undefined name `missing`
```
