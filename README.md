# Starry 编程语言

Starry 是一种基于 C/C++ 语言衍生的现代多范式编程语言，深度融合面向对象、泛型编程及函数式编程等理论范式。该语言设计遵循简约性、高效性与可维护性原则。

## 项目结构

```
starry/
├── CMakeLists.txt          # 主CMake构建文件
├── starry.g4               # ANTLR4语法定义文件
├── include/                # 公共头文件
│   └── starry/             # Starry语言头文件
├── src/                    # 源代码目录
│   ├── compiler/           # 编译器组件
│   │   ├── lexer/          # 词法分析器
│   │   ├── parser/         # 语法分析器
│   │   ├── ast/            # 抽象语法树
│   │   ├── semantic/       # 语义分析
│   │   └── codegen/        # 代码生成
│   ├── runtime/            # 运行时库
│   ├── stdlib/             # 标准库
│   └── main.cpp            # 编译器入口点
├── lib/                    # 第三方库
├── test/                   # 测试目录
│   ├── unit/               # 单元测试
│   ├── integration/        # 集成测试
│   └── performance/        # 性能测试
├── docs/                   # 文档
│   └── api/                # API文档
├── examples/               # 示例代码
└── tools/                  # 工具脚本
```

## 构建指南

### 前置条件

- CMake 4+
- C++20 兼容的编译器
- LLVM 20+
- ANTLR4 运行时库
- Python 3.6+ (用于构建脚本)

### 构建步骤

1. 克隆仓库

```bash
git clone https://github.com/your-org/starry.git
cd starry
```

2. 创建构建目录

```bash
mkdir build && cd build
```

3. 配置项目

```bash
cmake ..
```

4. 构建项目

```bash
cmake --build .
```

5. 运行测试

```bash
ctest
```

### 构建选项

- `-DCMAKE_BUILD_TYPE=Debug|Release|RelWithDebInfo` - 设置构建类型
- `-DENABLE_COVERAGE=ON|OFF` - 启用代码覆盖率报告
- `-DBUILD_SHARED_LIBS=ON|OFF` - 构建共享库而不是静态库
- `-DBUILD_TESTING=ON|OFF` - 构建测试

## 使用方法

### 编译 Starry 程序

```bash
./bin/starry_compiler -o output_file source_file.star
```

### 运行 Starry 程序

```bash
./output_file
```

## 开发指南

### 代码风格

- 使用4个空格缩进
- 使用驼峰命名法
- 类名使用大写字母开头
- 函数和变量名使用小写字母开头
- 常量使用全大写字母

### 提交规范

提交信息应遵循以下格式：

```
<类型>(<范围>): <描述>

<详细说明>

<关闭的问题>
```

类型可以是：
- feat: 新功能
- fix: 修复bug
- docs: 文档更改
- style: 不影响代码含义的更改（空格、格式等）
- refactor: 既不修复bug也不添加功能的代码更改
- perf: 提高性能的代码更改
- test: 添加或修正测试
- build: 影响构建系统或外部依赖的更改
- ci: 更改CI配置文件和脚本

### 分支策略

- `main`: 稳定分支，只接受经过测试的合并请求
- `dev`: 开发分支，新功能在此分支开发
- `feat/*`: 功能分支，用于开发新功能
- `bugfix/*`: 修复分支，用于修复bug
- `release/*`: 发布分支，用于准备新版本发布

## 贡献指南

1. Fork 仓库
2. 创建功能分支 (`git checkout -b feat/amazing-feature`)
3. 提交更改 (`git commit -m 'feat: add some amazing feature'`)
4. 推送到分支 (`git push origin feat/amazing-feature`)
5. 打开合并请求

## 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件


## 联系方式

- 项目维护者: [guangyiliushan](mailto:32023210165@cueb.edu.cn)
- 项目网站: "待开发"