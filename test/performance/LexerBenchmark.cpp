/**
 * @file LexerBenchmark.cpp
 * @brief Starry语言词法分析器性能测试
 * @author Starry Team
 * @date 2024
 */

#include <benchmark/benchmark.h>
#include "starry/Lexer.h"
#include <string>
#include <sstream>

using namespace starry::lexer;

// 基准测试：简单表达式词法分析
static void BM_LexerSimpleExpression(benchmark::State& state) {
    Lexer lexer;
    std::string source = "x + y * 2";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        benchmark::DoNotOptimize(tokens);
    }
}
BENCHMARK(BM_LexerSimpleExpression);

// 基准测试：复杂表达式词法分析
static void BM_LexerComplexExpression(benchmark::State& state) {
    Lexer lexer;
    std::string source = "(a + b) * (c - d) / (e % f) && (g || h)";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        benchmark::DoNotOptimize(tokens);
    }
}
BENCHMARK(BM_LexerComplexExpression);

// 基准测试：标识符词法分析
static void BM_LexerIdentifiers(benchmark::State& state) {
    Lexer lexer;
    std::string source = "variable_name another_var third_variable";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        benchmark::DoNotOptimize(tokens);
    }
}
BENCHMARK(BM_LexerIdentifiers);

// 基准测试：数字词法分析
static void BM_LexerNumbers(benchmark::State& state) {
    Lexer lexer;
    std::string source = "123 456.789 0.123 999";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        benchmark::DoNotOptimize(tokens);
    }
}
BENCHMARK(BM_LexerNumbers);

// 基准测试：字符串字面量词法分析
static void BM_LexerStrings(benchmark::State& state) {
    Lexer lexer;
    std::string source = R"("Hello World" "Another string" "Third string")";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        benchmark::DoNotOptimize(tokens);
    }
}
BENCHMARK(BM_LexerStrings);

// 基准测试：关键字词法分析
static void BM_LexerKeywords(benchmark::State& state) {
    Lexer lexer;
    std::string source = "if else while for function var let const";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        benchmark::DoNotOptimize(tokens);
    }
}
BENCHMARK(BM_LexerKeywords);

// 基准测试：大文件词法分析
static void BM_LexerLargeFile(benchmark::State& state) {
    Lexer lexer;
    
    // 生成大的源代码文件
    std::ostringstream oss;
    for (int i = 0; i < 1000; ++i) {
        oss << "var variable" << i << " = " << i << " + " << (i + 1) << ";\n";
    }
    std::string large_source = oss.str();
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(large_source);
        benchmark::DoNotOptimize(tokens);
    }
}
BENCHMARK(BM_LexerLargeFile);

// 基准测试：注释处理
static void BM_LexerComments(benchmark::State& state) {
    Lexer lexer;
    std::string source = R"(
        // 这是单行注释
        var x = 10; // 行末注释
        /* 这是多行注释
           跨越多行 */
        var y = 20;
    )";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        benchmark::DoNotOptimize(tokens);
    }
}
BENCHMARK(BM_LexerComments);

// 基准测试：空白字符处理
static void BM_LexerWhitespace(benchmark::State& state) {
    Lexer lexer;
    std::string source = "   var    x   =   10   +   20   ;   ";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        benchmark::DoNotOptimize(tokens);
    }
}
BENCHMARK(BM_LexerWhitespace);

// 基准测试：错误恢复
static void BM_LexerErrorRecovery(benchmark::State& state) {
    Lexer lexer;
    std::string source = "var x = 10; @invalid_char; var y = 20;";
    
    for (auto _ : state) {
        try {
            auto tokens = lexer.tokenize(source);
            benchmark::DoNotOptimize(tokens);
        } catch (...) {
            // 忽略错误，测试错误恢复性能
        }
    }
}
BENCHMARK(BM_LexerErrorRecovery);

// 基准测试：不同大小的输入
static void BM_LexerVariableSize(benchmark::State& state) {
    Lexer lexer;
    
    // 根据state.range(0)生成不同大小的输入
    std::ostringstream oss;
    for (int i = 0; i < state.range(0); ++i) {
        oss << "var x" << i << " = " << i << "; ";
    }
    std::string source = oss.str();
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        benchmark::DoNotOptimize(tokens);
    }
    
    state.SetComplexityN(state.range(0));
}
BENCHMARK(BM_LexerVariableSize)->Range(1, 1000)->Complexity();

// 基准测试：内存使用
static void BM_LexerMemoryUsage(benchmark::State& state) {
    Lexer lexer;
    std::string source = "var x = 10 + 20 * 30 / 40 - 50;";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        
        // 计算内存使用
        size_t memory_used = 0;
        for (const auto& token : tokens) {
            memory_used += sizeof(token) + token.getValue().size();
        }
        
        state.counters["MemoryUsed"] = memory_used;
        benchmark::DoNotOptimize(tokens);
    }
}
BENCHMARK(BM_LexerMemoryUsage);

// 基准测试：并发词法分析
static void BM_LexerConcurrent(benchmark::State& state) {
    Lexer lexer;
    std::string source = "var x = (a + b) * (c - d);";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        benchmark::DoNotOptimize(tokens);
    }
}
BENCHMARK(BM_LexerConcurrent)->Threads(4);

BENCHMARK_MAIN();