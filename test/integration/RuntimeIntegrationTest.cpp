/**
 * @file RuntimeIntegrationTest.cpp
 * @brief Starry语言运行时集成测试
 * @author Starry Team
 * @date 2024
 */

#include <gtest/gtest.h>
#include "starry/runtime/Runtime.h"
#include "starry/runtime/Memory.h"
#include <thread>
#include <chrono>

using namespace starry::runtime;

class RuntimeIntegrationTest : public ::testing::Test {
protected:
    void SetUp() override {
        // 每个测试开始前初始化运行时
        initialize_runtime();
    }
    
    void TearDown() override {
        // 每个测试结束后清理运行时
        cleanup_runtime();
    }
};

// 测试运行时系统初始化和清理
TEST_F(RuntimeIntegrationTest, InitializationTest) {
    // 运行时应该已经在SetUp中初始化
    EXPECT_TRUE(is_runtime_initialized());
    
    // 测试重复初始化
    initialize_runtime();
    EXPECT_TRUE(is_runtime_initialized());
}

// 测试程序执行
TEST_F(RuntimeIntegrationTest, ProgramExecutionTest) {
    ASSERT_TRUE(is_runtime_initialized());
    
    // 测试执行简单程序
    EXPECT_NO_THROW(execute_program("test_program"));
}

// 测试内存管理集成
TEST_F(RuntimeIntegrationTest, MemoryIntegrationTest) {
    ASSERT_TRUE(is_runtime_initialized());
    
    // 测试内存分配
    void* ptr1 = allocate_memory(1024);
    EXPECT_NE(ptr1, nullptr);
    
    void* ptr2 = allocate_memory(2048);
    EXPECT_NE(ptr2, nullptr);
    
    // 测试内存使用
    char* char_ptr = static_cast<char*>(ptr1);
    strcpy(char_ptr, "Hello Runtime");
    EXPECT_STREQ(char_ptr, "Hello Runtime");
    
    // 释放内存
    deallocate_memory(ptr1);
    deallocate_memory(ptr2);
}

// 测试运行时错误处理
TEST_F(RuntimeIntegrationTest, ErrorHandlingTest) {
    ASSERT_TRUE(is_runtime_initialized());
    
    // 测试运行时错误
    EXPECT_THROW(runtime_error("测试错误"), std::runtime_error);
    
    // 测试运行时警告（不应抛出异常）
    EXPECT_NO_THROW(runtime_warning("测试警告"));
}

// 测试并发安全性
TEST_F(RuntimeIntegrationTest, ConcurrencyTest) {
    ASSERT_TRUE(is_runtime_initialized());
    
    const int num_threads = 4;
    const int allocations_per_thread = 100;
    std::vector<std::thread> threads;
    std::vector<bool> results(num_threads, false);
    
    // 创建多个线程同时进行内存分配
    for (int i = 0; i < num_threads; ++i) {
        threads.emplace_back([&, i]() {
            try {
                std::vector<void*> pointers;
                
                // 分配内存
                for (int j = 0; j < allocations_per_thread; ++j) {
                    void* ptr = allocate_memory(64);
                    if (ptr) {
                        pointers.push_back(ptr);
                    }
                }
                
                // 使用内存
                for (void* ptr : pointers) {
                    if (ptr) {
                        char* char_ptr = static_cast<char*>(ptr);
                        char_ptr[0] = 'A' + i;
                        char_ptr[1] = '\0';
                    }
                }
                
                // 释放内存
                for (void* ptr : pointers) {
                    deallocate_memory(ptr);
                }
                
                results[i] = true;
            } catch (const std::exception& e) {
                results[i] = false;
            }
        });
    }
    
    // 等待所有线程完成
    for (auto& thread : threads) {
        thread.join();
    }
    
    // 检查所有线程都成功完成
    for (bool result : results) {
        EXPECT_TRUE(result);
    }
}

// 测试长时间运行
TEST_F(RuntimeIntegrationTest, LongRunningTest) {
    ASSERT_TRUE(is_runtime_initialized());
    
    const int iterations = 1000;
    const size_t alloc_size = 128;
    
    // 模拟长时间运行的程序
    for (int i = 0; i < iterations; ++i) {
        void* ptr = allocate_memory(alloc_size);
        if (ptr) {
            // 使用内存
            char* char_ptr = static_cast<char*>(ptr);
            snprintf(char_ptr, alloc_size, "Iteration %d", i);
            
            // 验证数据
            EXPECT_TRUE(strstr(char_ptr, "Iteration") != nullptr);
            
            // 释放内存
            deallocate_memory(ptr);
        }
        
        // 偶尔检查运行时状态
        if (i % 100 == 0) {
            EXPECT_TRUE(is_runtime_initialized());
        }
    }
}

// 测试内存压力
TEST_F(RuntimeIntegrationTest, MemoryStressTest) {
    ASSERT_TRUE(is_runtime_initialized());
    
    std::vector<void*> large_allocations;
    const size_t large_size = 64 * 1024; // 64KB
    const int max_allocations = 10;
    
    // 尝试分配大块内存
    for (int i = 0; i < max_allocations; ++i) {
        void* ptr = allocate_memory(large_size);
        if (ptr) {
            large_allocations.push_back(ptr);
            
            // 写入数据验证内存可用
            char* char_ptr = static_cast<char*>(ptr);
            char_ptr[0] = 'S';
            char_ptr[large_size - 1] = 'E';
            
            EXPECT_EQ(char_ptr[0], 'S');
            EXPECT_EQ(char_ptr[large_size - 1], 'E');
        }
    }
    
    // 释放所有大块内存
    for (void* ptr : large_allocations) {
        deallocate_memory(ptr);
    }
    
    // 验证运行时仍然正常
    EXPECT_TRUE(is_runtime_initialized());
}

// 测试运行时重启
TEST_F(RuntimeIntegrationTest, RuntimeRestartTest) {
    ASSERT_TRUE(is_runtime_initialized());
    
    // 分配一些内存
    void* ptr = allocate_memory(256);
    EXPECT_NE(ptr, nullptr);
    
    // 清理运行时
    cleanup_runtime();
    EXPECT_FALSE(is_runtime_initialized());
    
    // 重新初始化
    initialize_runtime();
    EXPECT_TRUE(is_runtime_initialized());
    
    // 测试新的内存分配
    void* new_ptr = allocate_memory(256);
    EXPECT_NE(new_ptr, nullptr);
    
    deallocate_memory(new_ptr);
}

// 测试异常恢复
TEST_F(RuntimeIntegrationTest, ExceptionRecoveryTest) {
    ASSERT_TRUE(is_runtime_initialized());
    
    // 触发运行时错误
    try {
        runtime_error("测试异常恢复");
        FAIL() << "应该抛出异常";
    } catch (const std::runtime_error& e) {
        // 异常被正确捕获
        EXPECT_TRUE(std::string(e.what()).find("测试异常恢复") != std::string::npos);
    }
    
    // 验证运行时仍然可用
    EXPECT_TRUE(is_runtime_initialized());
    
    // 测试正常操作仍然工作
    void* ptr = allocate_memory(128);
    EXPECT_NE(ptr, nullptr);
    deallocate_memory(ptr);
}

// 测试资源清理
TEST_F(RuntimeIntegrationTest, ResourceCleanupTest) {
    ASSERT_TRUE(is_runtime_initialized());
    
    std::vector<void*> pointers;
    const int num_allocations = 50;
    
    // 分配大量内存
    for (int i = 0; i < num_allocations; ++i) {
        void* ptr = allocate_memory(128);
        if (ptr) {
            pointers.push_back(ptr);
        }
    }
    
    // 只释放一半
    for (size_t i = 0; i < pointers.size() / 2; ++i) {
        deallocate_memory(pointers[i]);
    }
    
    // 运行时清理应该处理剩余的内存
    // 这在TearDown中会被调用
}

// 测试性能监控
TEST_F(RuntimeIntegrationTest, PerformanceMonitoringTest) {
    ASSERT_TRUE(is_runtime_initialized());
    
    const int iterations = 10000;
    auto start = std::chrono::high_resolution_clock::now();
    
    // 执行大量快速操作
    for (int i = 0; i < iterations; ++i) {
        void* ptr = allocate_memory(32);
        if (ptr) {
            deallocate_memory(ptr);
        }
    }
    
    auto end = std::chrono::high_resolution_clock::now();
    auto duration = std::chrono::duration_cast<std::chrono::microseconds>(end - start);
    
    // 性能应该在合理范围内
    std::cout << "运行时性能测试: " << iterations << " 次操作耗时 " 
              << duration.count() << " 微秒" << std::endl;
    
    // 平均每次操作不应超过100微秒
    EXPECT_LT(duration.count() / iterations, 100);
}