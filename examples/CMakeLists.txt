# 示例程序构建配置

# 词法分析器示例
add_executable(lexer_example
    lexer_test.cpp
)

target_link_libraries(lexer_example
    PRIVATE
    starry_lexer
    starry_cpp20_features
)

target_compile_features(lexer_example PRIVATE cxx_std_20)

# 语法分析器示例
add_executable(parser_example
    parser_test.cpp
)

target_link_libraries(parser_example
    PRIVATE
    starry_parser
    starry_ast
    starry_lexer
    starry_cpp20_features
)

target_compile_features(parser_example PRIVATE cxx_std_20)

# C++20特性测试示例
add_executable(cpp20_features_example
    cpp20_features_test.cpp
)

target_link_libraries(cpp20_features_example
    PRIVATE
    starry_parser
    starry_ast
    starry_lexer
    starry_cpp20_features
)

target_compile_features(cpp20_features_example PRIVATE cxx_std_20)

# 设置输出目录
set_target_properties(lexer_example parser_example cpp20_features_example
    PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin/examples
)