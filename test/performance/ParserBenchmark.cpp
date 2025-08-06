/**
 * @file ParserBenchmark.cpp
 * @brief Starry语言语法分析器性能测试
 * @author Starry Team
 * @date 2024
 */

#include <benchmark/benchmark.h>
#include "starry/Parser.h"
#include "starry/Lexer.h"
#include <string>
#include <sstream>

using namespace starry::parser;
using namespace starry::lexer;

// 基准测试：简单表达式语法分析
static void BM_ParserSimpleExpression(benchmark::State& state) {
    Lexer lexer;
    Parser parser;
    std::string source = "x + y";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        auto ast = parser.parseExpression(tokens);
        benchmark::DoNotOptimize(ast);
    }
}
BENCHMARK(BM_ParserSimpleExpression);

// 基准测试：复杂表达式语法分析
static void BM_ParserComplexExpression(benchmark::State& state) {
    Lexer lexer;
    Parser parser;
    std::string source = "(a + b) * (c - d) / (e % f)";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        auto ast = parser.parseExpression(tokens);
        benchmark::DoNotOptimize(ast);
    }
}
BENCHMARK(BM_ParserComplexExpression);

// 基准测试：嵌套表达式语法分析
static void BM_ParserNestedExpression(benchmark::State& state) {
    Lexer lexer;
    Parser parser;
    std::string source = "((a + b) * (c + d)) + ((e - f) / (g * h))";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        auto ast = parser.parseExpression(tokens);
        benchmark::DoNotOptimize(ast);
    }
}
BENCHMARK(BM_ParserNestedExpression);

// 基准测试：函数调用语法分析
static void BM_ParserFunctionCall(benchmark::State& state) {
    Lexer lexer;
    Parser parser;
    std::string source = "func(a, b, c)";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        auto ast = parser.parseExpression(tokens);
        benchmark::DoNotOptimize(ast);
    }
}
BENCHMARK(BM_ParserFunctionCall);

// 基准测试：嵌套函数调用语法分析
static void BM_ParserNestedFunctionCall(benchmark::State& state) {
    Lexer lexer;
    Parser parser;
    std::string source = "outer(inner(x, y), z)";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        auto ast = parser.parseExpression(tokens);
        benchmark::DoNotOptimize(ast);
    }
}
BENCHMARK(BM_ParserNestedFunctionCall);

// 基准测试：变量声明语法分析
static void BM_ParserVariableDeclaration(benchmark::State& state) {
    Lexer lexer;
    Parser parser;
    std::string source = "var x = 10;";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        auto ast = parser.parseStatement(tokens);
        benchmark::DoNotOptimize(ast);
    }
}
BENCHMARK(BM_ParserVariableDeclaration);

// 基准测试：函数定义语法分析
static void BM_ParserFunctionDefinition(benchmark::State& state) {
    Lexer lexer;
    Parser parser;
    std::string source = "function add(a, b) { return a + b; }";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        auto ast = parser.parseStatement(tokens);
        benchmark::DoNotOptimize(ast);
    }
}
BENCHMARK(BM_ParserFunctionDefinition);

// 基准测试：条件语句语法分析
static void BM_ParserIfStatement(benchmark::State& state) {
    Lexer lexer;
    Parser parser;
    std::string source = "if (x > 0) { return x; } else { return -x; }";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        auto ast = parser.parseStatement(tokens);
        benchmark::DoNotOptimize(ast);
    }
}
BENCHMARK(BM_ParserIfStatement);

// 基准测试：循环语句语法分析
static void BM_ParserWhileLoop(benchmark::State& state) {
    Lexer lexer;
    Parser parser;
    std::string source = "while (i < 10) { i = i + 1; }";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        auto ast = parser.parseStatement(tokens);
        benchmark::DoNotOptimize(ast);
    }
}
BENCHMARK(BM_ParserWhileLoop);

// 基准测试：大程序语法分析
static void BM_ParserLargeProgram(benchmark::State& state) {
    Lexer lexer;
    Parser parser;
    
    // 生成大程序
    std::ostringstream oss;
    oss << "function main() {\n";
    for (int i = 0; i < 100; ++i) {
        oss << "  var x" << i << " = " << i << " + " << (i + 1) << ";\n";
    }
    oss << "  return x99;\n";
    oss << "}\n";
    
    std::string source = oss.str();
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        auto ast = parser.parseProgram(tokens);
        benchmark::DoNotOptimize(ast);
    }
}
BENCHMARK(BM_ParserLargeProgram);

// 基准测试：错误恢复性能
static void BM_ParserErrorRecovery(benchmark::State& state) {
    Lexer lexer;
    Parser parser;
    std::string source = "var x = 10 + ; var y = 20;"; // 语法错误
    
    for (auto _ : state) {
        try {
            auto tokens = lexer.tokenize(source);
            auto ast = parser.parseProgram(tokens);
            benchmark::DoNotOptimize(ast);
        } catch (...) {
            // 忽略错误，测试错误恢复性能
        }
    }
}
BENCHMARK(BM_ParserErrorRecovery);

// 基准测试：不同复杂度的表达式
static void BM_ParserExpressionComplexity(benchmark::State& state) {
    Lexer lexer;
    Parser parser;
    
    // 根据state.range(0)生成不同复杂度的表达式
    std::ostringstream oss;
    oss << "x";
    for (int i = 1; i < state.range(0); ++i) {
        oss << " + x" << i;
    }
    
    std::string source = oss.str();
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        auto ast = parser.parseExpression(tokens);
        benchmark::DoNotOptimize(ast);
    }
    
    state.SetComplexityN(state.range(0));
}
BENCHMARK(BM_ParserExpressionComplexity)->Range(1, 100)->Complexity();

// 基准测试：内存使用
static void BM_ParserMemoryUsage(benchmark::State& state) {
    Lexer lexer;
    Parser parser;
    std::string source = "function test() { var x = (a + b) * (c - d); return x; }";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        auto ast = parser.parseStatement(tokens);
        
        // 估算AST内存使用
        size_t estimated_memory = sizeof(*ast) * 10; // 简单估算
        state.counters["EstimatedMemory"] = estimated_memory;
        
        benchmark::DoNotOptimize(ast);
    }
}
BENCHMARK(BM_ParserMemoryUsage);

// 基准测试：并发语法分析
static void BM_ParserConcurrent(benchmark::State& state) {
    Lexer lexer;
    Parser parser;
    std::string source = "var result = (a + b) * (c - d);";
    
    for (auto _ : state) {
        auto tokens = lexer.tokenize(source);
        auto ast = parser.parseStatement(tokens);
        benchmark::DoNotOptimize(ast);
    }
}
BENCHMARK(BM_ParserConcurrent)->Threads(4);

BENCHMARK_MAIN();