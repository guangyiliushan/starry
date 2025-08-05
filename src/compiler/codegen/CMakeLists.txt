# 代码生成器构建配置

# 创建代码生成库
add_library(starry_codegen
    CodeGenerator.cpp
    LLVMEmitter.cpp
    IRBuilder.cpp
    Optimizer.cpp
)

target_include_directories(starry_codegen
    PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${LLVM_INCLUDE_DIRS}
)

# 链接LLVM库
llvm_map_components_to_libnames(llvm_libs
    Core
    Support
    Native
    Analysis
    BitWriter
    ExecutionEngine
    InstCombine
    Object
    OrcJIT
    Passes
    RuntimeDyld
    ScalarOpts
    Target
    TransformUtils
)

target_link_libraries(starry_codegen
    PUBLIC
    starry_semantic
    ${llvm_libs}
)