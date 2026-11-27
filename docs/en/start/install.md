+++
title = "Install"
description = "Build the Starry compiler and verify the command-line tool."
+++

# Install

Starry uses Rust 2024 Edition and requires Rust 1.85.0 or later. Native examples also need `clang` or `llvm-as` on `PATH`.

Build the compiler:

```bash
cargo build -p starryc
```

Create a file named `hello.starry`:

```starry check
fun main(): int {
    print("hello")
    return 0
}
```

Verify the file:

```bash
cargo run -p starryc -- check hello.starry
```

The command exits `0` when the program is accepted.
