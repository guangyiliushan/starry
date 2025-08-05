# 词法分析器构建配置

# 使用ANTLR4生成词法分析器
find_package(ANTLR REQUIRED)
antlr_target(StarryLexer ${CMAKE_SOURCE_DIR}/starry.g4 
             LEXER
             PACKAGE starry)

# 创建词法分析器库
add_library(starry_lexer
    Lexer.cpp
    Token.cpp
    ${ANTLR_StarryLexer_OUTPUTS}
)

target_include_directories(starry_lexer
    PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${ANTLR_StarryLexer_OUTPUT_DIR}
)

target_link_libraries(starry_lexer
    PUBLIC
    antlr4_static
)

# 设置编译选项
target_compile_features(starry_lexer PUBLIC cxx_std_20)
