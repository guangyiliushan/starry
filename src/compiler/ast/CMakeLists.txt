# 抽象语法树(AST)构建配置

# 创建AST库
add_library(starry_ast
    ASTNode.cpp
    Expression.cpp
    Statement.cpp
    Declaration.cpp
    Type.cpp
    Visitor.cpp
)

target_include_directories(starry_ast
    PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}
)

target_link_libraries(starry_ast
    PUBLIC
    starry_parser
)