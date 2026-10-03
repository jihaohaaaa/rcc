# `rscc` — Rust C Compiler

`rscc` 是一个基于 Rust 语言实现的现代 C 语言编译器，支持 C99 / C11 核心规范，采用模块化架构，原生支持 **x86_64 (Linux / macOS AMD64)** 与 **AArch64 (Apple Silicon / Linux ARM64)** 汇编生成与执行。

## 特性概览

- **完整编译器管线**：
  - **C 预处理器 (`rscc::preprocessor`)**：支持 `#define`（对象宏与函数宏）、`#ifdef`、`#ifndef`、`#if`、`#else`、`#endif`、`#include`。
  - **词法分析器 (`rscc::lexer`)**：全字符集支持，包含多进制整数（十六进制、八进制、二进制）、浮点数、字符与字符串转义、各种 C 运算符。
  - **手写 Pratt 表达式与递归下降解析器 (`rscc::parser`)**：准确处理 C 运算符优先级与结合性，支持局部与全局声明、作用域 `typedef` 歧义消解。
  - **语义分析与类型系统 (`rscc::sema`, `rscc::types`)**：完整内存对齐与布局计算、指针算术与数组退化、结构体 (`struct`)、联合体 (`union`)、枚举 (`enum`)、类型转换与左值解析。
  - **多后端代码生成器 (`rscc::codegen`)**：
    - **x86_64 汇编生成器 (`rscc::codegen::x86_64`)**：遵循 System V AMD64 ABI 与 macOS AMD64 ABI（Intel 语法、`rdi`/`rsi`/`rdx`/`rcx`/`r8`/`r9` 传参、`xmm0`–`xmm7` 浮点传参、变长参数寄存器保存区）。
    - **AArch64 汇编生成器 (`rscc::codegen::aarch64`)**：遵循 AAPCS64 / Apple Silicon ABI（16 字节栈对齐、寄存器传参 `x0`~`x7`、Frame Pointer 栈帧保护）。
  - **CLI 驱动 (`rscc::driver`)**：支持 `-o`, `-c`, `-S`, `-E`, `-I`, `-D`, `--target`, `-m` 等标准选项，与系统工具链无缝配合。

## 快速使用

### 编译运行 C 程序
```bash
cargo run -- input.c -o myprog
./myprog
```

### 生成汇编代码 (`.s`)
```bash
# 自动匹配当前系统架构（x86_64 或 AArch64）
cargo run -- input.c -S

# 显式指定目标架构
cargo run -- input.c --target x86_64 -S
cargo run -- input.c --target aarch64 -S
```

### 仅运行预处理器 (`-E`)
```bash
cargo run -- input.c -E
```

## 测试

运行全套端到端集成测试：
```bash
cargo test
```
包含以下测试套件：
- `tests/expr_test.rs`：四则运算、位运算、逻辑运算、三元运算符、逗号表达式
- `tests/control_flow_test.rs`：`if/else`、`while`、`do-while`、`for`、`switch/case/default`、`break`、`continue`、`goto`
- `tests/function_test.rs`：多参数函数调用、递归调用（斐波那契、阶乘）
- `tests/pointer_test.rs`：指针读写、指针算术、单维与多维数组
- `tests/types_test.rs`：`sizeof`、`char`/`short`/`int`/`long`、结构体、枚举、`typedef`、全局变量
- `tests/preprocessor_test.rs`：宏定义与条件编译
