+++
title = "CLI"
description = "当前 Starry 命令行接口。"
+++

# CLI

## 命令

| 命令 | 用途 |
|---|---|
| `starryc match <regex> <input>` | 运行正则表达式探针并显示最长匹配长度。 |
| `starryc dump-nfa <regex>` | 打印 NFA 状态和转移。 |
| `starryc tokenize <source>` | 打印词法 token 流。 |
| `starryc check <file.starry>` | 解析文件并执行语义检查。 |
| `starryc irc <file.starry>` | 打印三地址码。 |
| `starryc opt <file.starry>` | 打印优化后的三地址码。 |
| `starryc llvme <file.starry>` | 打印 LLVM IR 文本。 |
| `starryc build <file.starry> [out]` | 编译并链接原生可执行文件。 |
| `starryc build <dir|starry.toml> [out]` | 从 manifest 或目录构建包。 |

## 构建参数

`starryc build` 中 `--` 之后的参数会传给原生工具链。对 `extern "c"` 调用，这些参数可以是目标文件、库名和库路径。

```bash
starryc build program.starry program -- math.obj -lm
```

## 退出码

`check`、`irc`、`opt`、`llvme` 和 `build` 在输入被接受时返回 `0`，在产生源码诊断时返回 `1`。

诊断使用以下形式：

```text
file.starry:line:column: Code: message
```
