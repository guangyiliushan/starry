+++
title = "教程"
description = "编写并运行第一个 Starry 程序。"
+++

# 教程

本教程构建 `starryc`，编写完整程序，并检查输出。

构建编译器：

```bash
cargo build -p starryc
```

## 值与插值

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

## 结构体与数组

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

## 类与释放时清理

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

## 抽象类

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

具体子类必须覆盖所有继承的抽象方法。抽象类不能实例化。

## Trait 与动态派发

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

被拒绝的程序会报告包含文件、行、列、稳定代码和消息的诊断。

```starry reject
fun main(): int {
    print(missing)
    return 0
}
```

```text
program.starry:2:11: SemUndefinedName: undefined name `missing`
```
