# 集成测试构建配置

# 包含GoogleTest
include(GoogleTest)

# 编译器端到端测试
add_executable(compiler_integration_test
    CompilerIntegrationTest.cpp
)
target_link_libraries(compiler_integration_test
    PRIVATE
    starry_lexer
    starry_parser
    starry_ast
    starry_semantic
    starry_codegen
    gtest
    gtest_main
)
gtest_discover_tests(compiler_integration_test)

# 运行时集成测试
add_executable(runtime_integration_test
    RuntimeIntegrationTest.cpp
)
target_link_libraries(runtime_integration_test
    PRIVATE
    starry_runtime_core
    starry_stdlib
    gtest
    gtest_main
)
gtest_discover_tests(runtime_integration_test)

# 标准库集成测试
add_executable(stdlib_integration_test
    StdlibIntegrationTest.cpp
)
target_link_libraries(stdlib_integration_test
    PRIVATE
    starry_stdlib
    gtest
    gtest_main
)
gtest_discover_tests(stdlib_integration_test)