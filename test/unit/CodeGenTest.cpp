/**
 * @file CodeGenTest.cpp
 * @brief Starry语言代码生成单元测试
 * @author Starry Team
 * @date 2024
 */

#include <gtest/gtest.h>
#include "starry/codegen/CodeGenerator.h"
#include "starry/AST.h"
#include <sstream>
#include <memory>

using namespace starry::codegen;
using namespace starry::ast;

class CodeGenTest : public ::testing::Test {
protected:
    std::unique_ptr<CodeGenerator> generator;
    std::ostringstream output;
    
    void SetUp() override {
        generator = std::make_unique<CodeGenerator>(output);
    }
    
    void TearDown() override {
        generator.reset();
        output.str("");
        output.clear();
    }
};

// 测试字面量代码生成
TEST_F(CodeGenTest, LiteralCodeGenTest) {
    auto literal = std::make_unique<LiteralExpression>("42", LiteralType::Integer);
    
    literal->accept(*generator);
    
    EXPECT_EQ(output.str(), "42");
}

// 测试标识符代码生成
TEST_F(CodeGenTest, IdentifierCodeGenTest) {
    auto identifier = std::make_unique<IdentifierExpression>("variable");
    
    identifier->accept(*generator);
    
    EXPECT_EQ(output.str(), "variable");
}

// 测试二元表达式代码生成
TEST_F(CodeGenTest, BinaryExpressionCodeGenTest) {
    auto left = std::make_unique<LiteralExpression>("10", LiteralType::Integer);
    auto right = std::make_unique<LiteralExpression>("20", LiteralType::Integer);
    auto binary = std::make_unique<BinaryExpression>(
        std::move(left), 
        BinaryOperator::Add, 
        std::move(right)
    );
    
    binary->accept(*generator);
    
    EXPECT_EQ(output.str(), "(10 + 20)");
}

// 测试一元表达式代码生成
TEST_F(CodeGenTest, UnaryExpressionCodeGenTest) {
    auto operand = std::make_unique<LiteralExpression>("42", LiteralType::Integer);
    auto unary = std::make_unique<UnaryExpression>(
        UnaryOperator::Minus, 
        std::move(operand)
    );
    
    unary->accept(*generator);
    
    EXPECT_EQ(output.str(), "-(42)");
}

// 测试函数调用代码生成
TEST_F(CodeGenTest, CallExpressionCodeGenTest) {
    auto callee = std::make_unique<IdentifierExpression>("print");
    std::vector<std::unique_ptr<Expression>> args;
    args.push_back(std::make_unique<LiteralExpression>("Hello", LiteralType::String));
    args.push_back(std::make_unique<LiteralExpression>("World", LiteralType::String));
    
    auto call = std::make_unique<CallExpression>(std::move(callee), std::move(args));
    
    call->accept(*generator);
    
    EXPECT_EQ(output.str(), "print(Hello, World)");
}

// 测试复杂表达式代码生成
TEST_F(CodeGenTest, ComplexExpressionCodeGenTest) {
    // 构建表达式: (a + b) * c
    auto a = std::make_unique<IdentifierExpression>("a");
    auto b = std::make_unique<IdentifierExpression>("b");
    auto c = std::make_unique<IdentifierExpression>("c");
    
    auto add = std::make_unique<BinaryExpression>(
        std::move(a), 
        BinaryOperator::Add, 
        std::move(b)
    );
    
    auto multiply = std::make_unique<BinaryExpression>(
        std::move(add), 
        BinaryOperator::Multiply, 
        std::move(c)
    );
    
    multiply->accept(*generator);
    
    EXPECT_EQ(output.str(), "((a + b) * c)");
}

// 测试所有二元运算符
TEST_F(CodeGenTest, AllBinaryOperatorsTest) {
    struct TestCase {
        BinaryOperator op;
        std::string expected;
    };
    
    std::vector<TestCase> test_cases = {
        {BinaryOperator::Add, " + "},
        {BinaryOperator::Subtract, " - "},
        {BinaryOperator::Multiply, " * "},
        {BinaryOperator::Divide, " / "},
        {BinaryOperator::Equal, " == "},
        {BinaryOperator::NotEqual, " != "},
        {BinaryOperator::Less, " < "},
        {BinaryOperator::Greater, " > "},
        {BinaryOperator::LessEqual, " <= "},
        {BinaryOperator::GreaterEqual, " >= "},
        {BinaryOperator::LogicalAnd, " && "},
        {BinaryOperator::LogicalOr, " || "}
    };
    
    for (const auto& test_case : test_cases) {
        output.str("");
        output.clear();
        
        auto left = std::make_unique<LiteralExpression>("a", LiteralType::Integer);
        auto right = std::make_unique<LiteralExpression>("b", LiteralType::Integer);
        auto binary = std::make_unique<BinaryExpression>(
            std::move(left), 
            test_case.op, 
            std::move(right)
        );
        
        binary->accept(*generator);
        
        std::string expected = "(a" + test_case.expected + "b)";
        EXPECT_EQ(output.str(), expected);
    }
}

// 测试所有一元运算符
TEST_F(CodeGenTest, AllUnaryOperatorsTest) {
    struct TestCase {
        UnaryOperator op;
        std::string expected;
    };
    
    std::vector<TestCase> test_cases = {
        {UnaryOperator::Plus, "+"},
        {UnaryOperator::Minus, "-"},
        {UnaryOperator::LogicalNot, "!"}
    };
    
    for (const auto& test_case : test_cases) {
        output.str("");
        output.clear();
        
        auto operand = std::make_unique<LiteralExpression>("x", LiteralType::Integer);
        auto unary = std::make_unique<UnaryExpression>(
            test_case.op, 
            std::move(operand)
        );
        
        unary->accept(*generator);
        
        std::string expected = test_case.expected + "(x)";
        EXPECT_EQ(output.str(), expected);
    }
}

// 测试嵌套函数调用
TEST_F(CodeGenTest, NestedCallExpressionTest) {
    // 构建表达式: outer(inner(x), y)
    auto x = std::make_unique<IdentifierExpression>("x");
    auto y = std::make_unique<IdentifierExpression>("y");
    
    auto inner_callee = std::make_unique<IdentifierExpression>("inner");
    std::vector<std::unique_ptr<Expression>> inner_args;
    inner_args.push_back(std::move(x));
    auto inner_call = std::make_unique<CallExpression>(std::move(inner_callee), std::move(inner_args));
    
    auto outer_callee = std::make_unique<IdentifierExpression>("outer");
    std::vector<std::unique_ptr<Expression>> outer_args;
    outer_args.push_back(std::move(inner_call));
    outer_args.push_back(std::move(y));
    auto outer_call = std::make_unique<CallExpression>(std::move(outer_callee), std::move(outer_args));
    
    outer_call->accept(*generator);
    
    EXPECT_EQ(output.str(), "outer(inner(x), y)");
}

// 测试函数生成
TEST_F(CodeGenTest, FunctionGenerationTest) {
    std::vector<std::string> parameters = {"param1", "param2"};
    auto body = std::make_unique<LiteralExpression>("return_value", LiteralType::Integer);
    
    generator->generateFunction("test_function", parameters, *body);
    
    std::string expected = "void test_function(auto param1, auto param2) {\nreturn_value\n}\n\n";
    EXPECT_EQ(output.str(), expected);
}

// 测试主函数生成
TEST_F(CodeGenTest, MainFunctionGenerationTest) {
    generator->generateMain();
    
    std::string expected = "int main() {\n    // 主函数代码\n    return 0;\n}\n";
    EXPECT_EQ(output.str(), expected);
}