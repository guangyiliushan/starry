# 语法分析器构建配置

# 使用ANTLR4生成语法分析器
find_package(ANTLR REQUIRED)
antlr_target(StarryParser ${CMAKE_SOURCE_DIR}/starry.g4 
             PARSER
             PACKAGE starry
             DEPENDS_ANTLR StarryLexer
             VISITOR)

# 创建语法分析器库
add_library(starry_parser
    Parser.cpp
    ${ANTLR_StarryParser_OUTPUTS}
)

target_include_directories(starry_parser
    PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${ANTLR_StarryParser_OUTPUT_DIR}
)

target_link_libraries(starry_parser
    PUBLIC
    starry_lexer
    antlr4_static
)