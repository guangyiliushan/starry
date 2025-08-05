# 性能测试构建配置

# 包含GoogleTest和Google Benchmark
include(GoogleTest)
find_package(benchmark QUIET)

# 如果找不到Google Benchmark，则使用FetchContent获取
if(NOT benchmark_FOUND)
    include(FetchContent)
    FetchContent_Declare(
        benchmark
        GIT_REPOSITORY https://github.com/google/benchmark.git
        GIT_TAG v1.6.1
    )
    set(BENCHMARK_ENABLE_TESTING OFF CACHE BOOL "" FORCE)
    FetchContent_MakeAvailable(benchmark)
endif()

# 词法分析器性能测试
add_executable(lexer_benchmark
    LexerBenchmark.cpp
)
target_link_libraries(lexer_benchmark
    PRIVATE
    starry_lexer
    benchmark::benchmark
)

# 语法分析器性能测试
add_executable(parser_benchmark
    ParserBenchmark.cpp
)
target_link_libraries(parser_benchmark
    PRIVATE
    starry_parser
    benchmark::benchmark
)

# 代码生成性能测试
add_executable(codegen_benchmark
    CodeGenBenchmark.cpp
)
target_link_libraries(codegen_benchmark
    PRIVATE
    starry_codegen
    benchmark::benchmark
)

# 运行时性能测试
add_executable(runtime_benchmark
    MemoryBenchmark.cpp
    GCBenchmark.cpp
)
target_link_libraries(runtime_benchmark
    PRIVATE
    starry_runtime_core
    benchmark::benchmark
)

# 标准库性能测试
add_executable(stdlib_benchmark
    StringBenchmark.cpp
    CollectionBenchmark.cpp
)
target_link_libraries(stdlib_benchmark
    PRIVATE
    starry_stdlib
    benchmark::benchmark
)