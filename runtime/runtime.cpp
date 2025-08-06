/**
 * @file runtime.cpp
 * @brief Starry语言运行时系统主文件
 * @author Starry Team
 * @date 2024
 */

#include "starry/runtime/Memory.h"
#include <iostream>
#include <cstdlib>

namespace starry {
namespace runtime {

/**
 * @brief 初始化Starry运行时系统
 * @return 成功返回0，失败返回非0值
 */
int initialize_runtime() {
    try {
        // 初始化内存管理系统
        initialize_memory(64 * 1024 * 1024); // 64MB默认内存池
        
        std::cout << "Starry运行时系统初始化成功" << std::endl;
        return 0;
    } catch (const std::exception& e) {
        std::cerr << "运行时初始化失败: " << e.what() << std::endl;
        return -1;
    }
}

/**
 * @brief 清理Starry运行时系统
 */
void cleanup_runtime() {
    try {
        // 清理内存管理系统
        cleanup_memory();
        
        std::cout << "Starry运行时系统清理完成" << std::endl;
    } catch (const std::exception& e) {
        std::cerr << "运行时清理失败: " << e.what() << std::endl;
    }
}

/**
 * @brief 运行时错误处理
 * @param error_code 错误代码
 * @param message 错误消息
 */
void handle_runtime_error(int error_code, const char* message) {
    std::cerr << "运行时错误 [" << error_code << "]: " << message << std::endl;
    
    // 根据错误类型决定是否终止程序
    if (error_code < 0) {
        std::cerr << "严重错误，程序即将退出" << std::endl;
        cleanup_runtime();
        std::exit(error_code);
    }
}

/**
 * @brief 获取运行时统计信息
 */
void print_runtime_stats() {
    std::cout << "=== Starry运行时统计信息 ===" << std::endl;
    
    // 内存使用统计
    size_t total_memory = get_total_memory();
    size_t used_memory = get_used_memory();
    size_t free_memory = total_memory - used_memory;
    
    std::cout << "总内存: " << total_memory << " 字节" << std::endl;
    std::cout << "已用内存: " << used_memory << " 字节" << std::endl;
    std::cout << "空闲内存: " << free_memory << " 字节" << std::endl;
    std::cout << "内存使用率: " << (double)used_memory / total_memory * 100 << "%" << std::endl;
    
    std::cout << "=========================" << std::endl;
}

} // namespace runtime
} // namespace starry

/**
 * @brief 运行时系统的C接口
 */
extern "C" {

int starry_runtime_init() {
    return starry::runtime::initialize_runtime();
}

void starry_runtime_cleanup() {
    starry::runtime::cleanup_runtime();
}

void starry_runtime_error(int code, const char* msg) {
    starry::runtime::handle_runtime_error(code, msg);
}

void starry_runtime_stats() {
    starry::runtime::print_runtime_stats();
}

} // extern "C"