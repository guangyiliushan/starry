# Starry编译器和运行时构建配置

# 编译器组件
add_subdirectory(compiler)

# 运行时库
add_subdirectory(runtime)

# 标准库
add_subdirectory(stdlib)

# 创建编译器可执行文件
add_executable(starry_compiler
    main.cpp
)

target_link_libraries(starry_compiler
    PRIVATE
    starry_lexer
    starry_parser
    starry_ast
    starry_semantic
    starry_codegen
)

# 创建运行时库
add_library(starry_runtime SHARED
    runtime/runtime.cpp
)

target_include_directories(starry_runtime
    PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/include
)

# 安装规则
install(TARGETS starry_compiler starry_runtime
    RUNTIME DESTINATION bin
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
)