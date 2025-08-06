# lib/CMakeLists.txt
# 第三方库和外部依赖配置

cmake_minimum_required(VERSION 3.16)

# 设置第三方库目录
set(LIB_DIR ${CMAKE_CURRENT_SOURCE_DIR})

# LLVM库配置（如果需要）
if(LLVM_FOUND)
    message(STATUS "配置LLVM库支持")
    # LLVM相关配置可以在这里添加
endif()

# ANTLR4运行时库配置
if(TARGET antlr4_static)
    message(STATUS "ANTLR4运行时库已配置")
endif()

# Google Test库配置
if(BUILD_TESTING)
    message(STATUS "配置Google Test库")
    # 如果有自定义的测试库配置可以在这里添加
endif()

# Google Benchmark库配置
if(BUILD_BENCHMARKS)
    message(STATUS "配置Google Benchmark库")
    # 如果有自定义的性能测试库配置可以在这里添加
endif()

# 其他第三方库
# 例如：JSON库、字符串处理库等
# add_subdirectory(json)
# add_subdirectory(fmt)

# 创建一个lib目标（如果需要）
# add_library(starry_libs INTERFACE)
# target_link_libraries(starry_libs INTERFACE ${LLVM_LIBRARIES})

message(STATUS "第三方库配置完成")
