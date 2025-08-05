# Starry编程语言项目结构

本文档详细描述了Starry编程语言项目的完整结构和组织方式。

## 项目概览

Starry是一种基于C/C++语言衍生的现代多范式编程语言，项目采用模块化设计，支持团队协作开发。

## 目录结构

```
starry/
├── CMakeLists.txt              # 主CMake构建配置文件
├── starry.g4                   # ANTLR4语法定义文件
├── build.py                    # Python构建脚本
├── LICENSE                     # MIT许可证文件
├── README.md                   # 项目说明文档
├── Development.md              # 开发流程文档
├── PROJECT_STRUCTURE.md        # 项目结构说明（本文件）
├── .gitignore                  # Git忽略规则
│
├── include/                    # 公共头文件目录
│   └── starry/                 # Starry语言头文件
│       └── Lexer.h             # 词法分析器头文件
│
├── src/                        # 源代码目录
│   ├── CMakeLists.txt          # 源代码构建配置
│   ├── main.cpp                # 编译器主程序入口
│   │
│   ├── compiler/               # 编译器组件
│   │   ├── CMakeLists.txt      # 编译器构建配置
│   │   │
│   │   ├── lexer/              # 词法分析器
│   │   │   ├── CMakeLists.txt  # 词法分析器构建配置
│   │   │   ├── Lexer.cpp       # 词法分析器实现
│   │   │   └── Token.cpp       # 词法单元实现
│   │   │
│   │   ├── parser/             # 语法分析器
│   │   │   ├── CMakeLists.txt  # 语法分析器构建配置
│   │   │   └── Parser.cpp      # 语法分析器实现（待实现）
│   │   │
│   │   ├── ast/                # 抽象语法树
│   │   │   ├── CMakeLists.txt  # AST构建配置
│   │   │   ├── ASTNode.cpp     # AST节点实现（待实现）
│   │   │   ├── Expression.cpp  # 表达式节点（待实现）
│   │   │   ├── Statement.cpp   # 语句节点（待实现）
│   │   │   ├── Declaration.cpp # 声明节点（待实现）
│   │   │   ├── Type.cpp        # 类型节点（待实现）
│   │   │   └── Visitor.cpp     # 访问者模式（待实现）
│   │   │
│   │   ├── semantic/           # 语义分析
│   │   │   ├── CMakeLists.txt  # 语义分析构建配置
│   │   │   ├── SymbolTable.cpp # 符号表（待实现）
│   │   │   ├── TypeChecker.cpp # 类型检查器（待实现）
│   │   │   ├── SemanticAnalyzer.cpp # 语义分析器（待实现）
│   │   │   ├── Scope.cpp       # 作用域管理（待实现）
│   │   │   └── Error.cpp       # 错误处理（待实现）
│   │   │
│   │   └── codegen/            # 代码生成
│   │       ├── CMakeLists.txt  # 代码生成构建配置
│   │       ├── CodeGenerator.cpp # 代码生成器（待实现）
│   │       ├── LLVMEmitter.cpp # LLVM代码发射器（待实现）
│   │       ├── IRBuilder.cpp   # 中间代码构建器（待实现）
│   │       └── Optimizer.cpp   # 优化器（待实现）
│   │
│   ├── runtime/                # 运行时库
│   │   ├── CMakeLists.txt      # 运行时构建配置
│   │   ├── Memory.cpp          # 内存管理（待实现）
│   │   ├── Thread.cpp          # 线程管理（待实现）
│   │   ├── Exception.cpp       # 异常处理（待实现）
│   │   ├── GC.cpp              # 垃圾回收（待实现）
│   │   └── IO.cpp              # 输入输出（待实现）
│   │
│   └── stdlib/                 # 标准库
│       ├── CMakeLists.txt      # 标准库构建配置
│       ├── String.cpp          # 字符串库（待实现）
│       ├── Collection.cpp      # 集合库（待实现）
│       ├── Math.cpp            # 数学库（待实现）
│       ├── IO.cpp              # IO库（待实现）
│       ├── System.cpp          # 系统库（待实现）
│       └── Network.cpp         # 网络库（待实现）
│
├── lib/                        # 第三方库目录
│
├── test/                       # 测试目录
│   ├── CMakeLists.txt          # 测试构建配置
│   │
│   ├── unit/                   # 单元测试
│   │   ├── CMakeLists.txt      # 单元测试构建配置
│   │   ├── LexerTest.cpp       # 词法分析器测试
│   │   ├── ParserTest.cpp      # 语法分析器测试（待实现）
│   │   ├── ASTTest.cpp         # AST测试（待实现）
│   │   ├── SemanticTest.cpp    # 语义分析测试（待实现）
│   │   ├── TypeCheckerTest.cpp # 类型检查测试（待实现）
│   │   ├── SymbolTableTest.cpp # 符号表测试（待实现）
│   │   ├── CodeGenTest.cpp     # 代码生成测试（待实现）
│   │   ├── MemoryTest.cpp      # 内存管理测试（待实现）
│   │   ├── ThreadTest.cpp      # 线程测试（待实现）
│   │   ├── ExceptionTest.cpp   # 异常测试（待实现）
│   │   ├── StringTest.cpp      # 字符串测试（待实现）
│   │   ├── CollectionTest.cpp  # 集合测试（待实现）
│   │   └── MathTest.cpp        # 数学测试（待实现）
│   │
│   ├── integration/            # 集成测试
│   │   ├── CMakeLists.txt      # 集成测试构建配置
│   │   ├── CompilerIntegrationTest.cpp # 编译器集成测试（待实现）
│   │   ├── RuntimeIntegrationTest.cpp  # 运行时集成测试（待实现）
│   │   └── StdlibIntegrationTest.cpp   # 标准库集成测试（待实现）
│   │
│   └── performance/            # 性能测试
│       ├── CMakeLists.txt      # 性能测试构建配置
│       ├── LexerBenchmark.cpp  # 词法分析器性能测试（待实现）
│       ├── ParserBenchmark.cpp # 语法分析器性能测试（待实现）
│       ├── CodeGenBenchmark.cpp # 代码生成性能测试（待实现）
│       ├── MemoryBenchmark.cpp # 内存管理性能测试（待实现）
│       ├── GCBenchmark.cpp     # 垃圾回收性能测试（待实现）
│       ├── StringBenchmark.cpp # 字符串性能测试（待实现）
│       └── CollectionBenchmark.cpp # 集合性能测试（待实现）
│
├── docs/                       # 文档目录
│   └── api/                    # API文档
│
├── examples/                   # 示例代码
│   ├── lexer_test.cpp          # 词法分析器测试示例
│   └── hello.star              # Starry语言Hello World示例
│
└── tools/                      # 工具脚本目录
```

## 核心组件说明

### 1. 编译器前端 (src/compiler/)

- **词法分析器 (lexer/)**：负责将源代码转换为词法单元序列
- **语法分析器 (parser/)**：基于ANTLR4生成，将词法单元转换为抽象语法树
- **抽象语法树 (ast/)**：表示程序结构的树形数据结构
- **语义分析 (semantic/)**：类型检查、符号表管理、作用域分析

### 2. 编译器后端 (src/compiler/codegen/)

- **代码生成器**：将AST转换为目标代码
- **LLVM集成**：使用LLVM框架进行代码优化和目标代码生成
- **中间代码**：支持多种中间表示形式

### 3. 运行时系统 (src/runtime/)

- **内存管理**：区域内存管理、垃圾回收
- **线程管理**：协程、并发原语
- **异常处理**：异常传播和处理机制

### 4. 标准库 (src/stdlib/)

- **基础类型**：字符串、集合、数学函数
- **系统接口**：文件IO、网络、系统调用
- **高级特性**：反射、序列化

## 构建系统

### CMake配置

项目使用CMake作为构建系统，支持：

- 多平台构建（Windows、Linux、macOS）
- 第三方依赖管理（LLVM、ANTLR4、GoogleTest）
- 静态/动态库生成
- 代码覆盖率报告
- 安装和打包

### Python构建脚本

`build.py`提供了便捷的构建接口：

```bash
# 完整构建流程
python build.py all

# 单独执行步骤
python build.py clean
python build.py configure --build-type Debug
python build.py build --jobs 4
python build.py test
python build.py coverage
```

## 测试框架

### 单元测试

- 使用GoogleTest框架
- 覆盖所有核心组件
- 自动化测试发现和执行

### 集成测试

- 端到端编译测试
- 运行时功能验证
- 标准库API测试

### 性能测试

- 使用Google Benchmark
- 编译器各阶段性能监控
- 运行时性能基准

## 开发工作流

### 代码规范

- C++17标准
- 4空格缩进
- 驼峰命名法
- 详细注释

### 版本控制

- Git分支策略：main/develop/feature/*
- 提交信息规范：type(scope): description
- 代码审查流程

### 持续集成

- 自动化构建和测试
- 代码质量检查
- 性能回归测试

## 依赖关系

### 必需依赖

- CMake 3.14+
- C++17编译器
- LLVM 12+
- ANTLR4运行时

### 可选依赖

- Google Test（测试）
- Google Benchmark（性能测试）
- Clang-format（代码格式化）
- Clang-tidy（静态分析）

## 扩展性设计

### 模块化架构

- 清晰的模块边界
- 最小化模块间依赖
- 插件式扩展机制

### 接口设计

- 抽象基类定义
- 策略模式应用
- 依赖注入支持

### 配置管理

- 编译时配置
- 运行时配置
- 环境变量支持

## 未来规划

### 短期目标

1. 完成基础编译器实现
2. 实现核心运行时功能
3. 建立完整的测试覆盖

### 中期目标

1. 优化编译器性能
2. 扩展标准库功能
3. 改进开发工具链

### 长期目标

1. 支持更多目标平台
2. 实现高级语言特性
3. 建立生态系统

## 贡献指南

欢迎开发者参与Starry语言的开发！请参考以下步骤：

1. Fork项目仓库
2. 创建功能分支
3. 实现功能并添加测试
4. 确保代码质量检查通过
5. 提交Pull Request

详细的贡献指南请参考README.md文件。