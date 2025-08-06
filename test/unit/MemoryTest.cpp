/**
 * @file MemoryTest.cpp
 * @brief Starry语言内存管理单元测试
 * @author Starry Team
 * @date 2024
 */

#include <gtest/gtest.h>
#include "starry/runtime/Memory.h"
#include <vector>
#include <cstring>

using namespace starry::runtime;

class MemoryTest : public ::testing::Test {
protected:
    void SetUp() override {
        // 每个测试开始前初始化内存系统
        initialize_memory(1024 * 1024); // 1MB内存池
    }
    
    void TearDown() override {
        // 每个测试结束后清理内存系统
        cleanup_memory();
    }
};

// 测试内存初始化和清理
TEST_F(MemoryTest, InitializationTest) {
    // 内存系统应该已经在SetUp中初始化
    // 测试分配一小块内存
    void* ptr = allocate_memory(100);
    EXPECT_NE(ptr, nullptr);
    
    // 释放内存
    deallocate_memory(ptr);
}

// 测试基本内存分配
TEST_F(MemoryTest, BasicAllocationTest) {
    // 分配不同大小的内存块
    void* ptr1 = allocate_memory(64);
    void* ptr2 = allocate_memory(128);
    void* ptr3 = allocate_memory(256);
    
    EXPECT_NE(ptr1, nullptr);
    EXPECT_NE(ptr2, nullptr);
    EXPECT_NE(ptr3, nullptr);
    
    // 确保指针不同
    EXPECT_NE(ptr1, ptr2);
    EXPECT_NE(ptr2, ptr3);
    EXPECT_NE(ptr1, ptr3);
    
    // 释放内存
    deallocate_memory(ptr1);
    deallocate_memory(ptr2);
    deallocate_memory(ptr3);
}

// 测试内存写入和读取
TEST_F(MemoryTest, MemoryReadWriteTest) {
    const size_t size = 256;
    void* ptr = allocate_memory(size);
    ASSERT_NE(ptr, nullptr);
    
    // 写入测试数据
    char* char_ptr = static_cast<char*>(ptr);
    const char* test_data = "Hello, Starry Memory System!";
    size_t test_len = strlen(test_data);
    
    memcpy(char_ptr, test_data, test_len + 1);
    
    // 读取并验证数据
    EXPECT_STREQ(char_ptr, test_data);
    
    deallocate_memory(ptr);
}

// 测试零大小分配
TEST_F(MemoryTest, ZeroSizeAllocationTest) {
    void* ptr = allocate_memory(0);
    // 零大小分配的行为可能因实现而异
    // 这里我们只是确保不会崩溃
    deallocate_memory(ptr);
}

// 测试空指针释放
TEST_F(MemoryTest, NullPointerDeallocationTest) {
    // 释放空指针应该是安全的
    deallocate_memory(nullptr);
    // 如果没有崩溃，测试通过
    SUCCEED();
}

// 测试大量小块分配
TEST_F(MemoryTest, ManySmallAllocationsTest) {
    const int num_allocations = 100;
    const size_t block_size = 64;
    std::vector<void*> pointers;
    
    // 分配大量小块
    for (int i = 0; i < num_allocations; ++i) {
        void* ptr = allocate_memory(block_size);
        if (ptr != nullptr) {
            pointers.push_back(ptr);
        }
    }
    
    // 至少应该能分配一些块
    EXPECT_GT(pointers.size(), 0);
    
    // 释放所有分配的内存
    for (void* ptr : pointers) {
        deallocate_memory(ptr);
    }
}

// 测试大块分配
TEST_F(MemoryTest, LargeAllocationTest) {
    const size_t large_size = 512 * 1024; // 512KB
    void* ptr = allocate_memory(large_size);
    
    if (ptr != nullptr) {
        // 如果分配成功，测试写入
        char* char_ptr = static_cast<char*>(ptr);
        char_ptr[0] = 'A';
        char_ptr[large_size - 1] = 'Z';
        
        EXPECT_EQ(char_ptr[0], 'A');
        EXPECT_EQ(char_ptr[large_size - 1], 'Z');
        
        deallocate_memory(ptr);
    }
    
    // 大块分配可能失败，这是正常的
    // 测试主要确保不会崩溃
}

// 测试内存重用
TEST_F(MemoryTest, MemoryReuseTest) {
    const size_t size = 128;
    
    // 分配并释放内存
    void* ptr1 = allocate_memory(size);
    ASSERT_NE(ptr1, nullptr);
    deallocate_memory(ptr1);
    
    // 再次分配相同大小的内存
    void* ptr2 = allocate_memory(size);
    EXPECT_NE(ptr2, nullptr);
    
    // 在理想情况下，内存池应该重用之前释放的内存
    // 但这不是必需的，所以我们只检查分配是否成功
    
    deallocate_memory(ptr2);
}

// 测试分配失败情况
TEST_F(MemoryTest, AllocationFailureTest) {
    // 尝试分配超大内存块，应该失败
    const size_t huge_size = 10 * 1024 * 1024; // 10MB，超过我们的1MB池
    void* ptr = allocate_memory(huge_size);
    
    // 分配应该失败
    EXPECT_EQ(ptr, nullptr);
}

// 测试内存对齐
TEST_F(MemoryTest, MemoryAlignmentTest) {
    const size_t size = 64;
    void* ptr = allocate_memory(size);
    
    if (ptr != nullptr) {
        // 检查指针是否合理对齐（至少是指针大小的倍数）
        uintptr_t addr = reinterpret_cast<uintptr_t>(ptr);
        EXPECT_EQ(addr % sizeof(void*), 0);
        
        deallocate_memory(ptr);
    }
}

// 测试重复初始化
TEST_F(MemoryTest, RepeatedInitializationTest) {
    // 重复初始化应该是安全的
    initialize_memory(512 * 1024);
    initialize_memory(256 * 1024);
    
    // 分配内存测试系统仍然工作
    void* ptr = allocate_memory(100);
    EXPECT_NE(ptr, nullptr);
    
    deallocate_memory(ptr);
}

// 性能测试：大量分配和释放
TEST_F(MemoryTest, PerformanceTest) {
    const int iterations = 1000;
    const size_t size = 32;
    
    auto start = std::chrono::high_resolution_clock::now();
    
    for (int i = 0; i < iterations; ++i) {
        void* ptr = allocate_memory(size);
        if (ptr != nullptr) {
            deallocate_memory(ptr);
        }
    }
    
    auto end = std::chrono::high_resolution_clock::now();
    auto duration = std::chrono::duration_cast<std::chrono::microseconds>(end - start);
    
    // 性能测试主要确保操作能在合理时间内完成
    // 这里我们只是记录时间，不做严格的性能要求
    std::cout << "内存分配/释放 " << iterations << " 次耗时: " 
              << duration.count() << " 微秒" << std::endl;
    
    SUCCEED();
}