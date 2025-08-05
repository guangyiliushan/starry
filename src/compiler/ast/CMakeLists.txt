# 抽象语法树(AST)构建配置

# 创建AST库
add_library(starry_ast
    AST.cpp
    ASTVisitor.cpp
)

target_include_directories(starry_ast
    PUBLIC
    ${CMAKE_SOURCE_DIR}/include
)

target_link_libraries(starry_ast
    PUBLIC
    starry_lexer
)

# 设置编译选项
target_compile_features(starry_ast PUBLIC cxx_std_20)
