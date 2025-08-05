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

# C++20特性支持检查
include(CheckCXXCompilerFlag)
check_cxx_compiler_flag("-std=c++20" COMPILER_SUPPORTS_CXX20)
if(NOT COMPILER_SUPPORTS_CXX20)
    message(FATAL_ERROR "编译器不支持C++20标准")
endif()

# 编译选项
if(MSVC)
    add_compile_options(/W4 /WX /MP /permissive- /std:c++20)
    # 启用C++20特性
    add_compile_definitions(_HAS_CXX20=1)
else()
    add_compile_options(-Wall -Wextra -Wpedantic -Werror -std=c++20)
    # 启用C++20特性支持
    add_compile_options(-fcoroutines -fconcepts)
endif()

# C++20模块支持（实验性）
option(ENABLE_CXX20_MODULES "启用C++20模块支持（实验性）" OFF)
if(ENABLE_CXX20_MODULES)
    if(CMAKE_CXX_COMPILER_ID MATCHES "Clang")
        add_compile_options(-fmodules-ts)
    elseif(CMAKE_CXX_COMPILER_ID MATCHES "GNU")
        add_compile_options(-fmodules-ts)
    endif()
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
add_subdirectory(examples)

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