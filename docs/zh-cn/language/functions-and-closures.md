+++
title = "函数与闭包"
description = "当前 Starry 的函数、实参、函数类型和闭包。"
+++

# 函数与闭包

## 函数

函数可以声明在顶层、类中和 trait 中。调用可以使用位置实参、命名实参或默认值。

```starry run
fun greet(name: string = "Starry", punctuation: string = "!"): string {
    return "hello ${name}${punctuation}"
}

fun main(): int {
    print(greet())
    print(greet("Ada"))
    print(greet(name = "Grace", punctuation = "?"))
    return 0
}
```

```text
hello Starry!
hello Ada!
hello Grace?
```

## 函数类型

函数类型列出参数类型和返回类型，例如 `(int) -> int`。

```starry run
fun twice(value: int): int {
    return value * 2
}

fun apply(operation: (int) -> int, value: int): int {
    return operation(value)
}

fun main(): int {
    print(apply(twice, 8))
    return 0
}
```

```text
16
```

## 闭包

闭包写在花括号中。参数在 `->` 前；无参数闭包可以使用 `it`。

```starry run
fun apply(operation: (int) -> int, value: int): int {
    return operation(value)
}

fun main(): int {
    print(apply({ n -> n * 3 }, 4))
    print(apply({ (number: int) -> number + 1 }, 4))
    return 0
}
```

```text
12
5
```

## 捕获值

闭包捕获它使用的值。被捕获的值在闭包体内继续可用。

```starry run
fun makeCounter(start: int): (int) -> int {
    return { (amount: int) -> start + amount }
}

fun main(): int {
    val advance = makeCounter(10)
    print(advance(2))
    print(advance(5))
    return 0
}
```

```text
12
15
```
