/**
 * @file CodeGenBenchmark.cpp
 * @brief Starry语言代码生成性能测试
 * @author Starry Team
 * @date 2024
 */

#include <benchmark/benchmark.h>
#include "starry/codegen/CodeGenerator.h"
#include "starry/AST.h"
#include <sstream>
#include <memory>

using namespace starry::codegen;
using namespace starry::ast;

// 基准测试：简单表达式代码生成
static void BM_CodeGenSimpleExpression(benchmark::State& state) {
    for (auto _ : state) {
        std::ostringstream output;
        CodeGenerator generator(output);
        
        auto left = std::make_unique<LiteralExpression>("10", LiteralType::Integer);
        auto right = std::make_unique<LiteralExpression>("20", LiteralType::Integer);
        auto binary = std::make_unique<BinaryExpression>(
            std::move(left), BinaryOperator::Add, std::move(right)
        );
        
        binary->accept(generator);
        benchmark::DoNotOptimize(output.str());
    }
}
BENCHMARK(BM_CodeGenSimpleExpression);

// 基准测试：复杂表达式代码生成
static void BM_CodeGenComplexExpression(benchmark::State& state) {
    for (auto _ : state) {
        std::ostringstream output;
        CodeGenerator generator(output);
        
        // 构建 (a + b) * (c - d)
        auto a = std::make_unique<IdentifierExpression>("a");
        auto b = std::make_unique<IdentifierExpression>("b");
        auto c = std::make_unique<IdentifierExpression>("c");
        auto d = std::make_unique<IdentifierExpression>("d");
        
        auto add = std::make_unique<BinaryExpression>(
            std::move(a), BinaryOperator::Add, std::move(b)
        );
        auto sub = std::make_unique<BinaryExpression>(
            std::move(c), BinaryOperator::Subtract, std::move(d)
        );
        auto mul = std::make_unique<BinaryExpression>(
            std::move(add), BinaryOperator::Multiply, std::move(sub)
        );
        
        mul->accept(generator);
        benchmark::DoNotOptimize(output.str());
    }
}
BENCHMARK(BM_CodeGenComplexExpression);

// 基准测试：函数调用代码生成
static void BM_CodeGenFunctionCall(benchmark::State& state) {
    for (auto _ : state) {
        std::ostringstream output;
        CodeGenerator generator(output);
        
        auto callee = std::make_unique<IdentifierExpression>("print");
        std::vector<std::unique_ptr<Expression>> args;
        args.push_back(std::make_unique<LiteralExpression>("Hello", LiteralType::String));
        args.push_back(std::make_unique<LiteralExpression>("World", LiteralType::String));
        
        auto call = std::make_unique<CallExpression>(std::move(callee), std::move(args));
        
        call->accept(generator);
        benchmark::DoNotOptimize(output.str());
    }
}
BENCHMARK(BM_CodeGenFunctionCall);

// 基准测试：嵌套表达式代码生成
static void BM_CodeGenNestedExpression(benchmark::State& state) {
    for (auto _ : state) {
        std::ostringstream output;
        CodeGenerator generator(output);
        
        // 构建 ((a + b) * c) + ((d - e) / f)
        auto a = std::make_unique<IdentifierExpression>("a");
        auto b = std::make_unique<IdentifierExpression>("b");
        auto c = std::make_unique<IdentifierExpression>("c");
        auto d = std::make_unique<IdentifierExpression>("d");
        auto e = std::make_unique<IdentifierExpression>("e");
        auto f = std::make_unique<IdentifierExpression>("f");
        
        auto add1 = std::make_unique<BinaryExpression>(
            std::move(a), BinaryOperator::Add, std::move(b)
        );
        auto mul = std::make_unique<BinaryExpression>(
            std::move(add1), BinaryOperator::Multiply, std::move(c)
        );
        
        auto sub = std::make_unique<BinaryExpression>(
            std::move(d), BinaryOperator::Subtract, std::move(e)
        );
        auto div = std::make_unique<BinaryExpression>(
            std::move(sub), BinaryOperator::Divide, std::move(f)
        );
        
        auto add2 = std::make_unique<BinaryExpression>(
            std::move(mul), BinaryOperator::Add, std::move(div)
        );
        
        add2->accept(generator);
        benchmark::DoNotOptimize(output.str());
    }
}
BENCHMARK(BM_CodeGenNestedExpression);

// 基准测试：大量变量代码生成
static void BM_CodeGenManyVariables(benchmark::State& state) {
    for (auto _ : state) {
        std::ostringstream output;
        CodeGenerator generator(output);
        
        // 生成大量变量的表达式
        auto expr = std::make_unique<IdentifierExpression>("x0");
        
        for (int i = 1; i < 50; ++i) {
            auto var = std::make_unique<IdentifierExpression>("x" + std::to_string(i));
            expr = std::make_unique<BinaryExpression>(
                std::move(expr), BinaryOperator::Add, std::move(var)
            );
        }
        
        expr->accept(generator);
        benchmark::DoNotOptimize(output.str());
    }
}
BENCHMARK(BM_CodeGenManyVariables);

// 基准测试：函数定义代码生成
static void BM_CodeGenFunctionDefinition(benchmark::State& state) {
    for (auto _ : state) {
        std::ostringstream output;
        CodeGenerator generator(output);
        
        std::vector<std::string> params = {"a", "b", "c"};
        auto body = std::make_unique<BinaryExpression>(
            std::make_unique<IdentifierExpression>("a"),
            BinaryOperator::Add,
            std::make_unique<IdentifierExpression>("b")
        );
        
        generator.generateFunction("test_func", params, *body);
        benchmark::DoNotOptimize(output.str());
    }
}
BENCHMARK(BM_CodeGenFunctionDefinition);

// 基准测试：主函数代码生成
static void BM_CodeGenMainFunction(benchmark::State& state) {
    for (auto _ : state) {
        std::ostringstream output;
        CodeGenerator generator(output);
        
        generator.generateMain();
        benchmark::DoNotOptimize(output.str());
    }
}
BENCHMARK(BM_CodeGenMainFunction);

// 基准测试：完整程序代码生成
static void BM_CodeGenCompleteProgram(benchmark::State& state) {
    for (auto _ : state) {
        std::ostringstream output;
        CodeGenerator generator(output);
        
        // 生成完整程序
        auto program = std::make_unique<LiteralExpression>("42", LiteralType::Integer);
        generator.generate(*program);
        
        benchmark::DoNotOptimize(output.str());
    }
}
BENCHMARK(BM_CodeGenCompleteProgram);

// 基准测试：不同复杂度的代码生成
static void BM_CodeGenComplexity(benchmark::State& state) {
    for (auto _ : state) {
        std::ostringstream output;
        CodeGenerator generator(output);
        
        // 根据state.range(0)生成不同复杂度的表达式
        auto expr = std::make_unique<LiteralExpression>("1", LiteralType::Integer);
        
        for (int i = 1; i < state.range(0); ++i) {
            auto literal = std::make_unique<LiteralExpression>(std::to_string(i + 1), LiteralType::Integer);
            expr = std::make_unique<BinaryExpression>(
                std::move(expr), BinaryOperator::Add, std::move(literal)
            );
        }
        
        expr->accept(generator);
        benchmark::DoNotOptimize(output.str());
    }
    
    state.SetComplexityN(state.range(0));
}
BENCHMARK(BM_CodeGenComplexity)->Range(1, 100)->Complexity();

// 基准测试：内存使用
static void BM_CodeGenMemoryUsage(benchmark::State& state) {
    for (auto _ : state) {
        std::ostringstream output;
        CodeGenerator generator(output);
        
        auto expr = std::make_unique<BinaryExpression>(
            std::make_unique<LiteralExpression>("10", LiteralType::Integer),
            BinaryOperator::Multiply,
            std::make_unique<LiteralExpression>("20", LiteralType::Integer)
        );
        
        expr->accept(generator);
        
        std::string result = output.str();
        state.counters["OutputSize"] = result.size();
        benchmark::DoNotOptimize(result);
    }
}
BENCHMARK(BM_CodeGenMemoryUsage);

// 基准测试：并发代码生成
static void BM_CodeGenConcurrent(benchmark::State& state) {
    for (auto _ : state) {
        std::ostringstream output;
        CodeGenerator generator(output);
        
        auto expr = std::make_unique<BinaryExpression>(
            std::make_unique<IdentifierExpression>("x"),
            BinaryOperator::Add,
            std::make_unique<IdentifierExpression>("y")
        );
        
        expr->accept(generator);
        benchmark::DoNotOptimize(output.str());
    }
}
BENCHMARK(BM_CodeGenConcurrent)->Threads(4);

// 基准测试：字符串输出性能
static void BM_CodeGenStringOutput(benchmark::State& state) {
    for (auto _ : state) {
        std::ostringstream output;
        CodeGenerator generator(output);
        
        // 生成大量字符串输出
        for (int i = 0; i < 100; ++i) {
            auto literal = std::make_unique<LiteralExpression>("string" + std::to_string(i), LiteralType::String);
            literal->accept(generator);
        }
        
        benchmark::DoNotOptimize(output.str());
    }
}
BENCHMARK(BM_CodeGenStringOutput);

BENCHMARK_MAIN();