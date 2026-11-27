+++
title = "语法"
description = "当前 Starry 的声明、表达式和词法形式。"
+++

# 语法

## 声明

`val` 创建不可变绑定。`var` 创建可变绑定。函数在返回值时显式声明参数和返回类型。

```starry run
fun add(left: int, right: int): int {
    return left + right
}

fun main(): int {
    val fixed = 10
    var changeable = 20
    changeable = 21
    print(fixed)
    print(changeable)
    print(add(4, 5))
    return 0
}
```

```text
10
21
9
```

## 字面量

整数支持十进制、十六进制、八进制和二进制形式。浮点字面量必须包含小数部分或指数。字符字面量是 Unicode。字符串可插值标识符和 `${}` 表达式。

```starry run
fun describe(count: int): string {
    return "count=${count}"
}

fun main(): int {
    val decimal = 42
    val hexadecimal = 0xff
    val octal = 0o17
    val binary = 0b101
    val real = 2.5
    val letter = 'A'
    val text = "value ${decimal}"
    print(decimal)
    print(hexadecimal)
    print(octal)
    print(binary)
    print(real)
    print(letter)
    print(text)
    print(describe(3))
    return 0
}
```

```text
42
255
15
5
2.5
A
value 42
count=3
```

## 表达式

表达式支持算术、比较、逻辑短路、取反和函数调用。

```starry check
fun main(): int {
    val sum = 7 + 2
    val product = 7 * 2
    val ordered = 1 < 2 && 2 < 3
    val alternative = false || true
    val inverted = !false
    print(sum)
    print(product)
    print(ordered)
    print(alternative)
    print(inverted)
    return 0
}
```

## 语句

换行结束语句。行尾是二元运算符，或下一行以只作为二元运算符的符号或 `.` 开头时，表达式会跨行继续。

```starry run
fun main(): int {
    val total = 1 +
        2
    print(total)
    return 0
}
```

```text
3
```
