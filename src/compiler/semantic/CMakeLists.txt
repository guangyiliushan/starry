# 语义分析器构建配置

# 创建语义分析库
add_library(starry_semantic
    SymbolTable.cpp
    TypeChecker.cpp
    SemanticAnalyzer.cpp
    Scope.cpp
    Error.cpp
)

target_include_directories(starry_semantic
    PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}
)

target_link_libraries(starry_semantic
    PUBLIC
    starry_ast
)