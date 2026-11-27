+++
title = "模块与包"
description = "当前 Starry 的导入、可见性、模块路径和包规则。"
+++

# 模块与包

Starry 没有按文件声明的包头。编译器根据包名和源文件路径推导模块路径。

## 导入

导入是显式的，并出现在源文件顶部。

```starry syntax
import util.helper
import geometry.shapes.*
import json.reader as reader
```

`name` 导入一个符号，`name.*` 导入模块命名空间，`name as alias` 创建本地别名。

## 可见性

声明可以限制在包或模块内。

```starry check
public fun visible(): int {
    return 1
}

fun main(): int {
    print(visible())
    return 0
}
```

```text
1
```

## 模块路径

一个目录是一个模块。在包中，manifest 名称是路径开头。目录名和包名使用点分标识符。

```text
math/lib.starry       -> math
math/src/vec/util.starry -> math.vec
demo/core/util.starry -> demo.core
```

## 包边界

一个包只能导入自己的源码和直接依赖。跨包导入只能使用 `public` 符号。依赖循环和导入循环会被拒绝。
