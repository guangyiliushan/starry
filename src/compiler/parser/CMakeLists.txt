find_package(antlr4-generator REQUIRED)
find_package(antlr4-runtime REQUIRED)
antlr4_generate(
    StarryParser
    ${CMAKE_SOURCE_DIR}/starry.g4
    PARSER
    FALSE
    FALSE
    "starry"
    "${ANTLR4_TOKEN_FILES_StarryLexer}"
    "${ANTLR4_TOKEN_DIRECTORY_StarryLexer}"
)
# 创建语法分析器库
add_library(starry_parser
    Parser.cpp
    ${ANTLR4_SRC_FILES_StarryParser}
)
add_dependencies(starry_parser ANTLR4_StarryParser starry_lexer)
target_include_directories(starry_parser PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${ANTLR4_INCLUDE_DIR_StarryParser}
)
target_link_libraries(starry_parser PUBLIC starry_lexer antlr4_shared)