# Starry quickstart tutorial

This walkthrough uses the local compiler CLI. You need Rust 1.85+ to build
`starryc`; native examples also need `clang` on `PATH` (or the standard Windows
LLVM install location).

```bash
cargo build -p starryc
cargo run -p starryc -- check program.starry
cargo run -p starryc -- build program.starry program
./program
```

The runnable regression corpus lives in `samples/`. Every top-level sample has a
matching golden stdout file in `samples/golden/`.

## 1. Values, control flow, and interpolation

```starry
fun main(): int {
    val name = "Starry"
    print("hello ${name}")
    if 1 + 1 == 2 {
        print("ok")
    }
    return 0
}
```

Expected stdout:

```text
hello Starry
ok
```

## 2. Structs, arrays, and field mutation

```starry
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

Expected stdout: `8`, then `7`.

## 3. Classes, ARC, and deinit

```starry
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

Expected stdout: `11`, then `11` when the last reference releases the object.

## 4. Abstract classes and inheritance

```starry
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

A concrete class must override every inherited abstract method. `abstract`
classes cannot be instantiated; `Square` supplies the runtime implementation.

## 5. Traits and `dyn` dispatch

```starry
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

Expected stdout: `16`.

## Diagnostics

Stable diagnostics look like:

```text
program.starry:3:5: SemUndefinedName: undefined name `missing`
```

Stable rejections are part of the language surface. See
`docs/syntax/feature-matrix.md` and `docs/syntax/phase4-scope.md` for the v0.1
capability boundary.
