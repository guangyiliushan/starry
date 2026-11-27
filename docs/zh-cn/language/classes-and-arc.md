+++
title = "类与 ARC"
description = "当前 Starry 的类、继承、构造器、弱引用和引用计数。"
+++

# 类与 ARC

Starry 对类实例使用自动引用计数。最后一个强引用消失时，类实例被释放。

## 构造器与方法

主构造参数标记 `val` 或 `var` 时成为字段。方法使用 `self` 访问实例状态。

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

## 释放时清理

`deinit` 在最后一个强引用释放实例时运行。

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

## 继承

`abstract` 声明不能实例化的类。`open` 允许子类化。`override` 提供继承的抽象方法。

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

## 弱引用

`weak var` 持有类实例但不让它保持存活。弱引用是可空的。

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
