+++
title = "诊断"
description = "当前稳定 Starry 诊断的最小示例。"
+++

# 诊断

诊断标识文件、行、列、稳定代码和消息。以下示例中的文件路径是 `program.starry`。

## 未定义名称

```starry reject
fun main(): int {
    print(missing)
    return 0
}
```

```text
program.starry:2:11: SemUndefinedName: undefined name `missing`
```

## 对 `val` 赋值

```starry reject
fun main(): int {
    val value = 1
    value = 2
    return 0
}
```

```text
program.starry:3:5: SemAssignToVal: cannot assign to an immutable binding
```

## 缺少实参

```starry reject
fun need(value: int): int {
    return value
}

fun main(): int {
    print(need())
    return 0
}
```

```text
program.starry:6:11: SemArityMismatch: missing argument for parameter `value`
```

## `when` 不穷尽

```starry reject
fun classify(value: int): int {
    return when value {
        0 -> 0
    }
}

fun main(): int {
    print(classify(0))
    return 0
}
```

```text
program.starry:2:12: SemMissingElse: when requires an else branch
```
