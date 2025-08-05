cmake_minimum_required(VERSION 3.14)

# 项目定义
project(Starry 
    VERSION 0.1.0
    DESCRIPTION "Starry编程语言编译器和运行时"
    LANGUAGES CXX C
)

# 设置C++标准
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

# 编译选项
if(MSVC)
    add_compile_options(/W4 /WX /MP /permissive-)
else()
    add_compile_options(-Wall -Wextra -Wpedantic -Werror)
endif()

# 输出目录设置
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/lib)
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)

# 包含目录
include_directories(${CMAKE_CURRENT_SOURCE_DIR}/include)

# 第三方依赖管理
include(FetchContent)

# LLVM依赖
find_package(LLVM REQUIRED CONFIG)
message(STATUS "Found LLVM ${LLVM_PACKAGE_VERSION}")
message(STATUS "Using LLVMConfig.cmake in: ${LLVM_DIR}")
include_directories(${LLVM_INCLUDE_DIRS})
add_definitions(${LLVM_DEFINITIONS})

# ANTLR4依赖
FetchContent_Declare(
    antlr4
    GIT_REPOSITORY https://github.com/antlr/antlr4.git
    GIT_TAG        4.9.3
    SOURCE_SUBDIR  runtime/Cpp
)
FetchContent_MakeAvailable(antlr4)

# GoogleTest依赖
FetchContent_Declare(
    googletest
    GIT_REPOSITORY https://github.com/google/googletest.git
    GIT_TAG        release-1.11.0
)
FetchContent_MakeAvailable(googletest)
enable_testing()

# 代码覆盖率支持
option(ENABLE_COVERAGE "启用代码覆盖率报告" OFF)
if(ENABLE_COVERAGE)
    if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
        add_compile_options(--coverage -O0)
        add_link_options(--coverage)
    endif()
endif()

# 添加子目录
add_subdirectory(src)
add_subdirectory(lib)
add_subdirectory(test)

# 安装规则
install(TARGETS starry_compiler starry_runtime
    RUNTIME DESTINATION bin
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
)
install(DIRECTORY include/ DESTINATION include)

# 打包配置
include(CPack)
set(CPACK_PACKAGE_VENDOR "Starry Language Team")
set(CPACK_PACKAGE_DESCRIPTION_SUMMARY "Starry编程语言")
set(CPACK_PACKAGE_VERSION_MAJOR ${PROJECT_VERSION_MAJOR})
set(CPACK_PACKAGE_VERSION_MINOR ${PROJECT_VERSION_MINOR})
set(CPACK_PACKAGE_VERSION_PATCH ${PROJECT_VERSION_PATCH})