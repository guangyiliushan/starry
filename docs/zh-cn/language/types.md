+++
title = "类型"
description = "当前 Starry 的标量、可空、聚合和用户定义类型。"
+++

# 类型

## 标量

Starry 提供整数、浮点数、布尔值、字符、字符串和 `unit`。整数在已知目标宽度时适配该宽度；否则保持在 `i64` 内或扩展为 `i128`。

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

## 可空值

尾部 `?` 表示可空类型。`null` 创建缺席值。`??` 提供替换值，`!!` 解包存在值。

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

## 数组与区间

数组字面量使用方括号，索引也使用方括号。区间可以用 `..` 表示排他，用 `..=` 表示包含。

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

## 结构体

结构体声明命名字段。主构造参数标记 `val` 或 `var` 时成为字段；无标记参数只在构造期使用。

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

## 枚举

枚举是一组封闭的变体。变体可以携带值。

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

## 元组

元组组合固定数量的值，并使用常量整数索引。

```starry check
fun main(): int {
    val pair = (1, "one")
    print(pair[0])
    print(pair[1])
    return 0
}
```
