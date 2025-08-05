# 语法分析器构建配置

# 创建语法分析器库
add_library(starry_parser
    Parser.cpp
)

target_include_directories(starry_parser
    PUBLIC
    ${CMAKE_SOURCE_DIR}/include
)

target_link_libraries(starry_parser
    PUBLIC
    starry_lexer
    starry_ast
)

# 设置编译选项
target_compile_features(starry_parser PUBLIC cxx_std_20)

# 可选：如果需要使用ANTLR4生成语法分析器
# find_package(ANTLR REQUIRED)
# antlr_target(StarryParser ${CMAKE_SOURCE_DIR}/starry.g4 
#              PARSER
#              PACKAGE starry
#              DEPENDS_ANTLR StarryLexer
#              VISITOR)
