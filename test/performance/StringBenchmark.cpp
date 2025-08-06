/**
 * @file StringBenchmark.cpp
 * @brief Starry语言字符串库性能测试
 * @author Starry Team
 * @date 2024
 */

#include <benchmark/benchmark.h>
#include "starry/stdlib/String.h"
#include "starry/runtime/Memory.h"
#include <string>
#include <vector>
#include <random>

using namespace starry::stdlib;
using namespace starry::runtime;

// 设置和清理函数
class StringFixture : public benchmark::Fixture {
public:
    void SetUp(const ::benchmark::State& state) override {
        initialize_memory(10 * 1024 * 1024); // 10MB内存池
    }
    
    void TearDown(const ::benchmark::State& state) override {
        cleanup_memory();
    }
};

// 基准测试：字符串构造
BENCHMARK_F(StringFixture, BM_StringConstruction)(benchmark::State& state) {
    for (auto _ : state) {
        StarryString str("Hello World");
        benchmark::DoNotOptimize(str);
    }
}

// 基准测试：字符串拷贝
BENCHMARK_F(StringFixture, BM_StringCopy)(benchmark::State& state) {
    StarryString original("This is a test string for copy benchmark");
    
    for (auto _ : state) {
        StarryString copy = original;
        benchmark::DoNotOptimize(copy);
    }
}

// 基准测试：字符串移动
BENCHMARK_F(StringFixture, BM_StringMove)(benchmark::State& state) {
    for (auto _ : state) {
        StarryString original("This is a test string for move benchmark");
        StarryString moved = std::move(original);
        benchmark::DoNotOptimize(moved);
    }
}

// 基准测试：字符串连接
BENCHMARK_F(StringFixture, BM_StringConcatenation)(benchmark::State& state) {
    StarryString str1("Hello");
    StarryString str2(" World");
    
    for (auto _ : state) {
        StarryString result = str1 + str2;
        benchmark::DoNotOptimize(result);
    }
}

// 基准测试：字符串连接赋值
BENCHMARK_F(StringFixture, BM_StringConcatenationAssignment)(benchmark::State& state) {
    StarryString str1("Hello");
    StarryString str2(" World");
    
    for (auto _ : state) {
        StarryString result = str1;
        result += str2;
        benchmark::DoNotOptimize(result);
    }
}

// 基准测试：字符串比较
BENCHMARK_F(StringFixture, BM_StringComparison)(benchmark::State& state) {
    StarryString str1("Hello World");
    StarryString str2("Hello World");
    StarryString str3("Different String");
    
    for (auto _ : state) {
        bool equal1 = (str1 == str2);
        bool equal2 = (str1 == str3);
        benchmark::DoNotOptimize(equal1);
        benchmark::DoNotOptimize(equal2);
    }
}

// 基准测试：字符串查找
BENCHMARK_F(StringFixture, BM_StringFind)(benchmark::State& state) {
    StarryString text("This is a long text with multiple occurrences of the word test in it");
    StarryString pattern("test");
    
    for (auto _ : state) {
        size_t pos = text.indexOf(pattern);
        benchmark::DoNotOptimize(pos);
    }
}

// 基准测试：字符串包含检查
BENCHMARK_F(StringFixture, BM_StringContains)(benchmark::State& state) {
    StarryString text("This is a long text with multiple words in it");
    StarryString pattern("multiple");
    
    for (auto _ : state) {
        bool contains = text.contains(pattern);
        benchmark::DoNotOptimize(contains);
    }
}

// 基准测试：字符串子串
BENCHMARK_F(StringFixture, BM_StringSubstring)(benchmark::State& state) {
    StarryString text("This is a very long string for substring testing purposes");
    
    for (auto _ : state) {
        StarryString sub = text.substring(10, 20);
        benchmark::DoNotOptimize(sub);
    }
}

// 基准测试：字符串大小写转换
BENCHMARK_F(StringFixture, BM_StringCaseConversion)(benchmark::State& state) {
    StarryString text("This Is A Mixed Case String For Testing");
    
    for (auto _ : state) {
        StarryString lower = text.toLowerCase();
        StarryString upper = text.toUpperCase();
        benchmark::DoNotOptimize(lower);
        benchmark::DoNotOptimize(upper);
    }
}

// 基准测试：字符串去空白
BENCHMARK_F(StringFixture, BM_StringTrim)(benchmark::State& state) {
    StarryString text("   This string has leading and trailing spaces   ");
    
    for (auto _ : state) {
        StarryString trimmed = text.trim();
        benchmark::DoNotOptimize(trimmed);
    }
}

// 基准测试：字符串分割
BENCHMARK_F(StringFixture, BM_StringSplit)(benchmark::State& state) {
    StarryString text("apple,banana,cherry,date,elderberry,fig,grape");
    StarryString delimiter(",");
    
    for (auto _ : state) {
        std::vector<StarryString> parts = text.split(delimiter);
        benchmark::DoNotOptimize(parts);
    }
}

// 基准测试：字符串替换
BENCHMARK_F(StringFixture, BM_StringReplace)(benchmark::State& state) {
    StarryString text("This is a test string with test words that need test replacement");
    StarryString from("test");
    StarryString to("TEST");
    
    for (auto _ : state) {
        StarryString result = text.replace(from, to);
        benchmark::DoNotOptimize(result);
    }
}

// 基准测试：字符串索引访问
BENCHMARK_F(StringFixture, BM_StringIndexAccess)(benchmark::State& state) {
    StarryString text("This is a string for index access testing");
    
    for (auto _ : state) {
        char sum = 0;
        for (size_t i = 0; i < text.length(); ++i) {
            sum += text[i];
        }
        benchmark::DoNotOptimize(sum);
    }
}

// 基准测试：类型转换
BENCHMARK_F(StringFixture, BM_TypeConversion)(benchmark::State& state) {
    for (auto _ : state) {
        StarryString int_str = toString(12345);
        StarryString double_str = toString(3.14159);
        StarryString bool_str = toString(true);
        
        int int_val = toInt(int_str);
        double double_val = toDouble(double_str);
        bool bool_val = toBool(bool_str);
        
        benchmark::DoNotOptimize(int_val);
        benchmark::DoNotOptimize(double_val);
        benchmark::DoNotOptimize(bool_val);
    }
}

// 基准测试：大字符串操作
BENCHMARK_F(StringFixture, BM_LargeStringOperations)(benchmark::State& state) {
    // 创建大字符串
    std::string large_content;
    for (int i = 0; i < 10000; ++i) {
        large_content += "This is line " + std::to_string(i) + " of the large string.\n";
    }
    StarryString large_string(large_content);
    
    for (auto _ : state) {
        StarryString upper = large_string.toUpperCase();
        benchmark::DoNotOptimize(upper);
    }
}

// 基准测试：字符串构建
BENCHMARK_F(StringFixture, BM_StringBuilding)(benchmark::State& state) {
    for (auto _ : state) {
        StarryString result;
        for (int i = 0; i < 100; ++i) {
            result += StarryString("Part ") + toString(i) + StarryString(" ");
        }
        benchmark::DoNotOptimize(result);
    }
}

// 基准测试：不同长度字符串的性能
BENCHMARK_F(StringFixture, BM_StringLengthPerformance)(benchmark::State& state) {
    size_t length = state.range(0);
    std::string content(length, 'A');
    StarryString test_string(content);
    
    for (auto _ : state) {
        StarryString copy = test_string;
        StarryString upper = copy.toUpperCase();
        benchmark::DoNotOptimize(upper);
    }
    
    state.SetBytesProcessed(state.iterations() * length);
}
BENCHMARK_F(StringFixture, BM_StringLengthPerformance)->Range(10, 10000)->RangeMultiplier(10);

// 基准测试：字符串搜索性能
BENCHMARK_F(StringFixture, BM_StringSearchPerformance)(benchmark::State& state) {
    // 创建包含重复模式的字符串
    std::string content;
    for (int i = 0; i < 1000; ++i) {
        content += "This is a test string with pattern. ";
    }
    StarryString text(content);
    StarryString pattern("pattern");
    
    for (auto _ : state) {
        size_t pos = text.indexOf(pattern);
        benchmark::DoNotOptimize(pos);
    }
}

// 基准测试：字符串内存使用
BENCHMARK_F(StringFixture, BM_StringMemoryUsage)(benchmark::State& state) {
    for (auto _ : state) {
        StarryString str("Memory usage test string");
        
        // 估算内存使用
        size_t memory_used = sizeof(str) + str.length();
        state.counters["MemoryUsed"] = memory_used;
        
        benchmark::DoNotOptimize(str);
    }
}

// 基准测试：并发字符串操作
BENCHMARK_F(StringFixture, BM_ConcurrentStringOperations)(benchmark::State& state) {
    StarryString text("Concurrent string operations test");
    
    for (auto _ : state) {
        StarryString copy = text;
        copy = copy.toUpperCase();
        copy = copy.toLowerCase();
        benchmark::DoNotOptimize(copy);
    }
}
BENCHMARK_F(StringFixture, BM_ConcurrentStringOperations)->Threads(4);

// 基准测试：字符串缓存效果
BENCHMARK_F(StringFixture, BM_StringCaching)(benchmark::State& state) {
    const int num_strings = 1000;
    std::vector<StarryString> strings;
    
    // 预分配字符串
    for (int i = 0; i < num_strings; ++i) {
        strings.emplace_back("Cached string " + std::to_string(i % 10)); // 重复模式
    }
    
    for (auto _ : state) {
        // 访问所有字符串
        size_t total_length = 0;
        for (const auto& str : strings) {
            total_length += str.length();
        }
        benchmark::DoNotOptimize(total_length);
    }
}

// 基准测试：字符串格式化性能
BENCHMARK_F(StringFixture, BM_StringFormatting)(benchmark::State& state) {
    StarryString template_str("Hello {name}, you have {count} messages and {percent}% completion");
    
    for (auto _ : state) {
        StarryString result = template_str;
        result = result.replace(StarryString("{name}"), StarryString("Alice"));
        result = result.replace(StarryString("{count}"), toString(42));
        result = result.replace(StarryString("{percent}"), toString(85));
        benchmark::DoNotOptimize(result);
    }
}

// 基准测试：随机字符串操作
BENCHMARK_F(StringFixture, BM_RandomStringOperations)(benchmark::State& state) {
    std::random_device rd;
    std::mt19937 gen(rd());
    std::uniform_int_distribution<> op_dis(0, 4);
    
    StarryString base_string("Random operations test string with some content");
    
    for (auto _ : state) {
        StarryString str = base_string;
        
        int operation = op_dis(gen);
        switch (operation) {
            case 0:
                str = str.toUpperCase();
                break;
            case 1:
                str = str.toLowerCase();
                break;
            case 2:
                str = str.substring(5, 15);
                break;
            case 3:
                str = str.replace(StarryString("test"), StarryString("TEST"));
                break;
            case 4:
                str = str + StarryString(" appended");
                break;
        }
        
        benchmark::DoNotOptimize(str);
    }
}

BENCHMARK_MAIN();