+++
title = "Classes and ARC"
description = "Current Starry classes, inheritance, constructors, weak references, and reference counting."
+++

# Classes and ARC

Starry uses automatic reference counting for class instances. A class instance is released when its last strong reference disappears.

## Constructors and methods

Primary-constructor parameters marked `val` or `var` become fields. Methods use `self` to access instance state.

```starry run
class Counter(var count: int) {
    fun bump(): int {
        self.count += 1
        return self.count
    }
}

fun main(): int {
    val counter = Counter(9)
    print(counter.bump())
    print(counter.bump())
    return 0
}
```

```text
10
11
```

## Release-time cleanup

`deinit` runs when the last strong reference releases the instance.

```starry run
class Resource(var name: string) {
    deinit {
        print("release")
    }
}

fun main(): int {
    val resource = Resource("file")
    return 0
}
```

```text
release
```

## Inheritance

`abstract` declares a class that cannot be instantiated. `open` permits subclassing. `override` supplies an inherited abstract method.

```starry run
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

```text
16
```

## Weak references

`weak var` holds a class instance without keeping it alive. A weak reference is nullable.

```starry run
class Node(var value: int) {
}

fun main(): int {
    val strong = Node(7)
    weak var weak = strong
    print(weak!!.value)
    return 0
}
```

```text
7
```
