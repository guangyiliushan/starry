+++
title = "控制流与模式"
description = "当前 Starry 的分支、循环、标签、defer 和模式。"
+++

# 控制流与模式

## 分支

`if` 以布尔值分支。`when` 将一个主体与多个模式比较；当结果被使用时必须穷尽。

```starry run
fun classify(value: int): string {
    return when value {
        0 -> "zero",
        1 -> "one",
        else -> "many",
    }
}

fun main(): int {
    if 1 < 2 {
        print("ordered")
    }
    print(classify(0))
    print(classify(1))
    print(classify(9))
    return 0
}
```

```text
ordered
zero
one
many
```

## 模式

模式从构造器、数组、元组和结构体中绑定值。`is` 在条件中测试一个模式；`when` 测试多个分支。

```starry run
enum Value {
    Number(int),
    Text(string),
}

fun main(): int {
    val value = Value.Number(4)
    if value is Value.Number(n) {
        print(n)
    }
    return 0
}
```

```text
4
```

## 循环

`for` 遍历数组和区间。`while` 在条件成立时重复。`loop` 重复直到 `break`。

```starry run
fun main(): int {
    for value in 1..=3 {
        if value == 2 {
            continue
        }
        print(value)
    }

    var count = 2
    while count > 0 {
        print(count)
        count = count - 1
    }

    loop {
        print("once")
        break
    }
    return 0
}
```

```text
1
3
2
1
once
```

## 标签与值

标签命名一个循环。`break LABEL VALUE` 携带值退出该循环。

```starry run
fun pick(): int {
    'outer: loop {
        break 'outer 42
    }
}

fun main(): int {
    print(pick())
    return 0
}
```

```text
42
```

## 延迟执行

`defer` 在所在作用域结束时按逆序运行。

```starry run
fun work(): int {
    defer {
        print("cleanup")
    }
    print("work")
    return 0
}

fun main(): int {
    return work()
}
```

```text
work
cleanup
```
