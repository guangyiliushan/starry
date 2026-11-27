+++
title = "Trait 与泛型"
description = "当前 Starry 的 trait、动态派发和泛型单态化。"
+++

# Trait 与泛型

## Trait

trait 声明必需方法。`impl` 为一个具体类型提供这些方法。trait 方法默认静态派发。

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

## 动态派发

`dyn Trait` 值在运行时通过 trait 实现派发。

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

## 泛型函数

类型参数写在函数名之后。调用可以显式提供类型实参，也可以使用推断。

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

## 泛型类型

结构体和枚举可以带类型参数。类型实参会被单态化，因此每个具体实例都有自己的布局。

```starry check
struct Box<T>(val value: T)

fun main(): int {
    val box = Box<int>(7)
    print(box.value)
    return 0
}
```
