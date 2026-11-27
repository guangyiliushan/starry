+++
title = "包"
description = "当前 Starry 的包 manifest、依赖、可见性和循环规则。"
+++

# 包

包由 `starry.toml` 标识。没有 manifest 的目录仍然是隐式单根构建。

## Manifest

```toml
[package]
name = "app"
version = "1.0.0"

[dependencies]
math = { path = "../math", version = "1.2.0" }
```

规则：

- `[package].name` 和 `[package].version` 是必需的非空字符串。
- `[dependencies]` 可选。裸依赖键是包名。
- 每个依赖内联表只包含 `path` 和 `version`。
- `path` 相对于依赖包的规范化根目录解析。
- `version` 与目标 `[package].version` 精确字符串匹配。
- 依赖键必须等于目标包的 `package.name`。
- 字符串转义、未知键、重复表、重复键、尾逗号和已知表之外的键都会以 `file:line:col` 诊断拒绝。

## 包图

包身份是包名。规范化根目录区分身份，因此来自两个不同根目录的相同包名是重复包错误。

包图存储 manifest、依赖规范、规范化根目录和依赖边。遍历是确定性的且按依赖优先。源文件按图后序收集，然后按路径排序。

## 导入与可见性

导入是显式且非传递的。包只能导入自己的源码和直接依赖。跨包导入只能使用 `public` 符号。`internal` 保持包内可见。`private` 和 `protected` 遵循源码级规则。

依赖循环会以完整确定性路径拒绝，例如 `dependency cycle: a -> b -> a`。导入循环和自导入也会以同样方式拒绝。

## 模块路径

模块路径是编译器元数据，不是源码语法。包中的路径以 manifest 名称开头，后接相对源码根的目录路径。一个目录是一个模块。目录名和包名使用点分标识符。

```text
math/lib.starry          -> math
math/src/vec/util.starry -> math.vec
demo/core/util.starry    -> demo.core
```
