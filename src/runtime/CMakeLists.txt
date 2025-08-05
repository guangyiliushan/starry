# 运行时库构建配置

# 创建运行时核心库
add_library(starry_runtime_core
    Memory.cpp
    Thread.cpp
    Exception.cpp
    GC.cpp
    IO.cpp
)

target_include_directories(starry_runtime_core
    PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${CMAKE_SOURCE_DIR}/include
)

# 安装规则
install(TARGETS starry_runtime_core
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
)