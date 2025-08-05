# 标准库构建配置

# 创建标准库
add_library(starry_stdlib
    String.cpp
    Collection.cpp
    Math.cpp
    IO.cpp
    System.cpp
    Network.cpp
)

target_include_directories(starry_stdlib
    PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${CMAKE_SOURCE_DIR}/include
)

target_link_libraries(starry_stdlib
    PUBLIC
    starry_runtime_core
)

# 安装规则
install(TARGETS starry_stdlib
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
)