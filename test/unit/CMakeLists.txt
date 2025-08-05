# 单元测试构建配置

# 包含GoogleTest
include(GoogleTest)

# 词法分析器测试
add_executable(lexer_test
    LexerTest.cpp
)
target_link_libraries(lexer_test
    PRIVATE
    starry_lexer
    gtest
    gtest_main
)
gtest_discover_tests(lexer_test)

# 语法分析器测试
add_executable(parser_test
    ParserTest.cpp
)
target_link_libraries(parser_test
    PRIVATE
    starry_parser
    gtest
    gtest_main
)
gtest_discover_tests(parser_test)

# AST测试
add_executable(ast_test
    ASTTest.cpp
)
target_link_libraries(ast_test
    PRIVATE
    starry_ast
    gtest
    gtest_main
)
gtest_discover_tests(ast_test)

# 语义分析测试
add_executable(semantic_test
    SemanticTest.cpp
    TypeCheckerTest.cpp
    SymbolTableTest.cpp
)
target_link_libraries(semantic_test
    PRIVATE
    starry_semantic
    gtest
    gtest_main
)
gtest_discover_tests(semantic_test)

# 代码生成测试
add_executable(codegen_test
    CodeGenTest.cpp
)
target_link_libraries(codegen_test
    PRIVATE
    starry_codegen
    gtest
    gtest_main
)
gtest_discover_tests(codegen_test)

# 运行时测试
add_executable(runtime_test
    MemoryTest.cpp
    ThreadTest.cpp
    ExceptionTest.cpp
)
target_link_libraries(runtime_test
    PRIVATE
    starry_runtime_core
    gtest
    gtest_main
)
gtest_discover_tests(runtime_test)

# 标准库测试
add_executable(stdlib_test
    StringTest.cpp
    CollectionTest.cpp
    MathTest.cpp
)
target_link_libraries(stdlib_test
    PRIVATE
    starry_stdlib
    gtest
    gtest_main
)
gtest_discover_tests(stdlib_test)