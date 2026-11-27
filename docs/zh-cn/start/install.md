+++
title = "安装"
description = "构建 Starry 编译器并验证命令行工具。"
+++

# 安装

Starry 使用 Rust 2024 Edition，需要 Rust 1.85.0 或更高版本。原生示例还需要 `PATH` 中有 `clang` 或 `llvm-as`。

构建编译器：

```bash
cargo build -p starryc
```

创建 `hello.starry`：

```starry check
fun main(): int {
    print("hello")
    return 0
}
```

验证该文件：

```bash
cargo run -p starryc -- check hello.starry
```

命令在程序被接受时返回 `0`。
