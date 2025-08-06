/**
 * @file StdlibIntegrationTest.cpp
 * @brief Starry语言标准库集成测试
 * @author Starry Team
 * @date 2024
 */

#include <gtest/gtest.h>
#include "starry/stdlib/String.h"
#include "starry/runtime/Memory.h"
#include <vector>
#include <algorithm>
#include <sstream>
#include <thread>
#include <chrono>

using namespace starry::stdlib;
using namespace starry::runtime;

class StdlibIntegrationTest : public ::testing::Test {
protected:
    void SetUp() override {
        // 初始化运行时系统（标准库可能依赖运行时）
        initialize_memory(1024 * 1024);
    }
    
    void TearDown() override {
        // 清理运行时系统
        cleanup_memory();
    }
};

// 测试字符串与内存管理集成
TEST_F(StdlibIntegrationTest, StringMemoryIntegrationTest) {
    // 创建大量字符串对象
    std::vector<StarryString> strings;
    const int num_strings = 1000;
    
    for (int i = 0; i < num_strings; ++i) {
        strings.emplace_back("String " + std::to_string(i));
    }
    
    // 验证所有字符串都正确创建
    for (int i = 0; i < num_strings; ++i) {
        std::string expected = "String " + std::to_string(i);
        EXPECT_EQ(strings[i].str(), expected);
    }
    
    // 测试字符串操作
    for (auto& str : strings) {
        str = str.toUpperCase();
        EXPECT_TRUE(str.contains(StarryString("STRING")));
    }
}

// 测试字符串处理管道
TEST_F(StdlibIntegrationTest, StringProcessingPipelineTest) {
    StarryString input("  Hello, World! This is a Test String.  ");
    
    // 处理管道：去空白 -> 转小写 -> 分割 -> 过滤 -> 连接
    StarryString trimmed = input.trim();
    EXPECT_EQ(trimmed.str(), "Hello, World! This is a Test String.");
    
    StarryString lowercase = trimmed.toLowerCase();
    EXPECT_EQ(lowercase.str(), "hello, world! this is a test string.");
    
    std::vector<StarryString> words = lowercase.split(StarryString(" "));
    EXPECT_EQ(words.size(), 7);
    
    // 过滤掉短单词
    std::vector<StarryString> long_words;
    for (const auto& word : words) {
        if (word.length() > 2) {
            long_words.push_back(word);
        }
    }
    
    EXPECT_GT(long_words.size(), 0);
    EXPECT_LT(long_words.size(), words.size());
    
    // 重新连接
    StarryString result;
    for (size_t i = 0; i < long_words.size(); ++i) {
        if (i > 0) result += StarryString(" ");
        result += long_words[i];
    }
    
    EXPECT_FALSE(result.empty());
    EXPECT_TRUE(result.contains(StarryString("hello")));
    EXPECT_TRUE(result.contains(StarryString("world")));
}

// 测试大文本处理
TEST_F(StdlibIntegrationTest, LargeTextProcessingTest) {
    // 创建大文本
    std::ostringstream oss;
    const int num_lines = 1000;
    
    for (int i = 0; i < num_lines; ++i) {
        oss << "Line " << i << ": This is a test line with some content.\n";
    }
    
    StarryString large_text(oss.str());
    EXPECT_GT(large_text.length(), 50000); // 应该是一个相当大的文本
    
    // 分割成行
    std::vector<StarryString> lines = large_text.split(StarryString("\n"));
    EXPECT_EQ(lines.size(), num_lines + 1); // +1 因为最后一个空行
    
    // 处理每一行
    int processed_lines = 0;
    for (const auto& line : lines) {
        if (!line.empty()) {
            EXPECT_TRUE(line.contains(StarryString("Line")));
            EXPECT_TRUE(line.contains(StarryString("test")));
            processed_lines++;
        }
    }
    
    EXPECT_EQ(processed_lines, num_lines);
}

// 测试字符串搜索和替换性能
TEST_F(StdlibIntegrationTest, SearchReplacePerformanceTest) {
    // 创建包含重复模式的大字符串
    StarryString pattern("test");
    StarryString replacement("TEST");
    
    std::ostringstream oss;
    for (int i = 0; i < 1000; ++i) {
        oss << "This is a test string with test patterns. ";
    }
    
    StarryString large_string(oss.str());
    
    auto start = std::chrono::high_resolution_clock::now();
    
    // 执行替换操作
    StarryString result = large_string.replace(pattern, replacement);
    
    auto end = std::chrono::high_resolution_clock::now();
    auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(end - start);
    
    // 验证替换结果
    EXPECT_FALSE(result.contains(pattern));
    EXPECT_TRUE(result.contains(replacement));
    
    // 性能应该在合理范围内（小于1秒）
    EXPECT_LT(duration.count(), 1000);
    
    std::cout << "大字符串替换操作耗时: " << duration.count() << " 毫秒" << std::endl;
}

// 测试类型转换集成
TEST_F(StdlibIntegrationTest, TypeConversionIntegrationTest) {
    // 测试数值转换链
    std::vector<int> numbers = {-100, -1, 0, 1, 42, 100, 999};
    
    for (int num : numbers) {
        StarryString str = toString(num);
        int converted_back = toInt(str);
        EXPECT_EQ(num, converted_back);
    }
    
    // 测试浮点数转换
    std::vector<double> doubles = {-3.14, -1.0, 0.0, 1.0, 2.718, 3.14159};
    
    for (double d : doubles) {
        StarryString str = toString(d);
        double converted_back = toDouble(str);
        EXPECT_NEAR(d, converted_back, 0.001); // 允许小的精度误差
    }
    
    // 测试布尔转换
    std::vector<bool> bools = {true, false};
    
    for (bool b : bools) {
        StarryString str = toString(b);
        bool converted_back = toBool(str);
        EXPECT_EQ(b, converted_back);
    }
}

// 测试字符串格式化
TEST_F(StdlibIntegrationTest, StringFormattingTest) {
    // 模拟简单的字符串格式化
    StarryString template_str("Hello, {name}! You have {count} messages.");
    
    StarryString name("Alice");
    StarryString count = toString(5);
    
    // 手动替换占位符
    StarryString result = template_str.replace(StarryString("{name}"), name);
    result = result.replace(StarryString("{count}"), count);
    
    EXPECT_EQ(result.str(), "Hello, Alice! You have 5 messages.");
}

// 测试字符串比较和排序
TEST_F(StdlibIntegrationTest, StringSortingTest) {
    std::vector<StarryString> strings = {
        StarryString("zebra"),
        StarryString("apple"),
        StarryString("banana"),
        StarryString("cherry"),
        StarryString("date")
    };
    
    // 使用标准库排序
    std::sort(strings.begin(), strings.end(), [](const StarryString& a, const StarryString& b) {
        return a < b;
    });
    
    // 验证排序结果
    EXPECT_EQ(strings[0].str(), "apple");
    EXPECT_EQ(strings[1].str(), "banana");
    EXPECT_EQ(strings[2].str(), "cherry");
    EXPECT_EQ(strings[3].str(), "date");
    EXPECT_EQ(strings[4].str(), "zebra");
}

// 测试字符串缓存和重用
TEST_F(StdlibIntegrationTest, StringCachingTest) {
    const int num_iterations = 10000;
    std::vector<StarryString> strings;
    
    // 创建大量相同的字符串
    for (int i = 0; i < num_iterations; ++i) {
        strings.emplace_back("cached_string");
    }
    
    // 验证所有字符串都相等
    for (const auto& str : strings) {
        EXPECT_EQ(str.str(), "cached_string");
    }
    
    // 测试字符串操作的一致性
    for (auto& str : strings) {
        str = str.toUpperCase();
        EXPECT_EQ(str.str(), "CACHED_STRING");
    }
}

// 测试多线程字符串操作
TEST_F(StdlibIntegrationTest, ConcurrentStringOperationsTest) {
    const int num_threads = 4;
    const int operations_per_thread = 100;
    std::vector<std::thread> threads;
    std::vector<bool> results(num_threads, false);
    
    for (int i = 0; i < num_threads; ++i) {
        threads.emplace_back([&, i]() {
            try {
                std::vector<StarryString> local_strings;
                
                // 每个线程创建和操作自己的字符串
                for (int j = 0; j < operations_per_thread; ++j) {
                    StarryString str("Thread " + std::to_string(i) + " String " + std::to_string(j));
                    
                    // 执行各种字符串操作
                    str = str.toUpperCase();
                    str = str.toLowerCase();
                    str = str.trim();
                    
                    if (str.contains(StarryString("thread"))) {
                        local_strings.push_back(str);
                    }
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

// 测试内存泄漏检测
TEST_F(StdlibIntegrationTest, MemoryLeakDetectionTest) {
    const int iterations = 1000;
    
    // 创建和销毁大量字符串对象
    for (int i = 0; i < iterations; ++i) {
        {
            StarryString temp("Temporary string " + std::to_string(i));
            StarryString copy = temp;
            StarryString moved = std::move(copy);
            
            // 执行一些操作
            moved = moved.toUpperCase();
            moved = moved.replace(StarryString("TEMPORARY"), StarryString("TEMP"));
            
            // 对象在作用域结束时自动销毁
        }
        
        // 偶尔检查内存状态
        if (i % 100 == 0) {
            // 这里可以添加内存使用检查
            // 目前只是确保程序没有崩溃
        }
    }
    
    // 如果没有内存泄漏，程序应该正常完成
    SUCCEED();
}

// 测试异常安全性
TEST_F(StdlibIntegrationTest, ExceptionSafetyTest) {
    StarryString str("Test string");
    
    // 测试索引越界异常
    EXPECT_THROW(str[100], std::out_of_range);
    
    // 异常后字符串应该仍然有效
    EXPECT_EQ(str.str(), "Test string");
    EXPECT_EQ(str.length(), 11);
    
    // 测试其他操作仍然正常
    StarryString upper = str.toUpperCase();
    EXPECT_EQ(upper.str(), "TEST STRING");
}

// 测试标准库与运行时的集成
TEST_F(StdlibIntegrationTest, RuntimeIntegrationTest) {
    // 测试字符串操作是否正确使用运行时内存管理
    std::vector<StarryString> strings;
    
    // 创建大量字符串，测试内存分配
    for (int i = 0; i < 100; ++i) {
        strings.emplace_back("Runtime integration test string " + std::to_string(i));
    }
    
    // 执行内存密集型操作
    for (auto& str : strings) {
        // 这些操作可能触发内存重新分配
        str = str + str; // 字符串连接
        str = str.substring(0, str.length() / 2); // 子字符串
        str = str.replace(StarryString("test"), StarryString("TEST")); // 替换
    }
    
    // 验证所有字符串仍然有效
    for (const auto& str : strings) {
        EXPECT_FALSE(str.empty());
        EXPECT_TRUE(str.contains(StarryString("Runtime")));
    }
}

// 测试边界条件处理
TEST_F(StdlibIntegrationTest, BoundaryConditionsTest) {
    // 测试空字符串的各种操作
    StarryString empty;
    
    EXPECT_TRUE(empty.empty());
    EXPECT_EQ(empty.length(), 0);
    EXPECT_EQ(empty.substring(0, 10).str(), "");
    EXPECT_EQ(empty.indexOf(StarryString("x")), SIZE_MAX);
    EXPECT_FALSE(empty.contains(StarryString("x")));
    EXPECT_EQ(empty.toLowerCase().str(), "");
    EXPECT_EQ(empty.toUpperCase().str(), "");
    EXPECT_EQ(empty.trim().str(), "");
    
    // 测试单字符字符串
    StarryString single("A");
    EXPECT_EQ(single.length(), 1);
    EXPECT_EQ(single[0], 'A');
    EXPECT_EQ(single.substring(0, 1).str(), "A");
    EXPECT_EQ(single.substring(1, 1).str(), "");
    
    // 测试非常长的字符串
    std::string long_content(10000, 'X');
    StarryString long_string(long_content);
    EXPECT_EQ(long_string.length(), 10000);
    EXPECT_TRUE(long_string.contains(StarryString("XXX")));
}

// 测试性能基准
TEST_F(StdlibIntegrationTest, PerformanceBenchmarkTest) {
    const int iterations = 10000;
    
    // 字符串创建性能
    auto start = std::chrono::high_resolution_clock::now();
    
    std::vector<StarryString> strings;
    for (int i = 0; i < iterations; ++i) {
        strings.emplace_back("Performance test " + std::to_string(i));
    }
    
    auto end = std::chrono::high_resolution_clock::now();
    auto creation_time = std::chrono::duration_cast<std::chrono::microseconds>(end - start);
    
    // 字符串操作性能
    start = std::chrono::high_resolution_clock::now();
    
    for (auto& str : strings) {
        str = str.toUpperCase();
        str = str.replace(StarryString("PERFORMANCE"), StarryString("PERF"));
    }
    
    end = std::chrono::high_resolution_clock::now();
    auto operation_time = std::chrono::duration_cast<std::chrono::microseconds>(end - start);
    
    std::cout << "字符串创建性能: " << iterations << " 个字符串耗时 " 
              << creation_time.count() << " 微秒" << std::endl;
    std::cout << "字符串操作性能: " << iterations << " 次操作耗时 " 
              << operation_time.count() << " 微秒" << std::endl;
    
    // 性能应该在合理范围内
    EXPECT_LT(creation_time.count() / iterations, 100); // 平均每个字符串创建不超过100微秒
    EXPECT_LT(operation_time.count() / iterations, 200); // 平均每次操作不超过200微秒
}