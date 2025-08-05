# Starry编译器组件构建配置

# 创建C++20特性支持库
add_library(starry_cpp20_features
    Cpp20Features.cpp
)

target_include_directories(starry_cpp20_features
    PUBLIC
    ${CMAKE_SOURCE_DIR}/include
)

target_compile_features(starry_cpp20_features PUBLIC cxx_std_20)

# 添加子目录
add_subdirectory(lexer)
add_subdirectory(parser)
add_subdirectory(ast)
add_subdirectory(semantic)
add_subdirectory(codegen)
