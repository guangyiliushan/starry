find_package(antlr4-generator REQUIRED)
find_package(antlr4-runtime REQUIRED)
antlr4_generate(
    StarryLexer
    ${CMAKE_SOURCE_DIR}/starry.g4
    LEXER
    FALSE
    FALSE
    "starry"
)
# 创建词法分析器库
add_library(starry_lexer
    Lexer.cpp
    Token.cpp
    ${ANTLR4_SRC_FILES_StarryLexer}
)
# add_dependencies(starry_lexer antlr4_generate_StarryLexer)
target_include_directories(starry_lexer PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${ANTLR4_INCLUDE_DIR_StarryLexer}
)
target_link_libraries(starry_lexer PUBLIC antlr4_shared)  # 或 antlr4_static，根据需求