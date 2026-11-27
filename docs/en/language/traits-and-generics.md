+++
title = "Traits and generics"
description = "Current Starry traits, dynamic dispatch, and generic monomorphization."
+++

# Traits and generics

## Traits

A trait declares required methods. `impl` supplies them for one concrete type. Trait methods are statically dispatched by default.

```starry run
trait Describe {
    fun describe(): string
}

class Named(val name: string) {
}

impl Describe for Named {
    fun describe(): string {
        return self.name
    }
}

fun main(): int {
    val value = Named("Starry")
    print(value.describe())
    return 0
}
```

```text
Starry
```

## Dynamic dispatch

A `dyn Trait` value dispatches through the trait implementation at run time.

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

## Generic functions

Type parameters are written after a function name. A call may supply type arguments explicitly or let inference choose them.

```starry run
fun first<T>(values: [T; 2], index: int): T {
    return values[index]
}

fun main(): int {
    print(first<int>([10, 20], 1))
    return 0
}
```

```text
20
```

## Generic types

Structs and enums can take type parameters. Type arguments are monomorphized, so each concrete instance has its own layout.

```starry check
struct Box<T>(val value: T)

fun main(): int {
    val box = Box<int>(7)
    print(box.value)
    return 0
}
```
