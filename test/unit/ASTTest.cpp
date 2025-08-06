/**
 * @file ASTTest.cpp
 * @brief Starry语言AST单元测试
 * @author Starry Team
 * @date 2024
 */

#include <gtest/gtest.h>
#include "starry/AST.h"
#include <memory>

using namespace starry::ast;

class ASTTest : public ::testing::Test {
protected:
    void SetUp() override {
        // 测试设置
    }
    
    void TearDown() override {
        // 测试清理
    }
};

// 测试字面量表达式
TEST_F(ASTTest, LiteralExpressionTest) {
    auto literal = std::make_unique<LiteralExpression>("42", LiteralType::Integer);
    
    EXPECT_EQ(literal->getValue(), "42");
    EXPECT_EQ(literal->getType(), LiteralType::Integer);
    EXPECT_EQ(literal->toString(), "Literal(42)");
}

// 测试标识符表达式
TEST_F(ASTTest, IdentifierExpressionTest) {
    auto identifier = std::make_unique<IdentifierExpression>("variable");
    
    EXPECT_EQ(identifier->getName(), "variable");
    EXPECT_EQ(identifier->toString(), "Identifier(variable)");
}

// 测试二元表达式
TEST_F(ASTTest, BinaryExpressionTest) {
    auto left = std::make_unique<LiteralExpression>("10", LiteralType::Integer);
    auto right = std::make_unique<LiteralExpression>("20", LiteralType::Integer);
    auto binary = std::make_unique<BinaryExpression>(
        std::move(left), 
        BinaryOperator::Add, 
        std::move(right)
    );
    
    EXPECT_EQ(binary->getOperator(), BinaryOperator::Add);
    EXPECT_EQ(binary->toString(), "Binary(Literal(10) + Literal(20))");
}

// 测试一元表达式
TEST_F(ASTTest, UnaryExpressionTest) {
    auto operand = std::make_unique<LiteralExpression>("42", LiteralType::Integer);
    auto unary = std::make_unique<UnaryExpression>(
        UnaryOperator::Minus, 
        std::move(operand)
    );
    
    EXPECT_EQ(unary->getOperator(), UnaryOperator::Minus);
    EXPECT_EQ(unary->toString(), "Unary(-Literal(42))");
}

// 测试函数调用表达式
TEST_F(ASTTest, CallExpressionTest) {
    auto callee = std::make_unique<IdentifierExpression>("print");
    std::vector<std::unique_ptr<Expression>> args;
    args.push_back(std::make_unique<LiteralExpression>("Hello", LiteralType::String));
    
    auto call = std::make_unique<CallExpression>(std::move(callee), std::move(args));
    
    EXPECT_EQ(call->toString(), "Call(Identifier(print)(Literal(Hello)))");
}

// 测试复杂表达式
TEST_F(ASTTest, ComplexExpressionTest) {
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
    
    EXPECT_EQ(multiply->toString(), "Binary(Binary(Identifier(a) + Identifier(b)) * Identifier(c))");
}

// 测试AST访问者模式
class TestVisitor : public ASTVisitor {
public:
    int visit_count = 0;
    
    void visit(const LiteralExpression& node) override {
        visit_count++;
    }
    
    void visit(const IdentifierExpression& node) override {
        visit_count++;
    }
    
    void visit(const BinaryExpression& node) override {
        visit_count++;
        node.getLeft().accept(*this);
        node.getRight().accept(*this);
    }
    
    void visit(const UnaryExpression& node) override {
        visit_count++;
        node.getOperand().accept(*this);
    }
    
    void visit(const CallExpression& node) override {
        visit_count++;
        node.getCallee().accept(*this);
        for (const auto& arg : node.getArguments()) {
            arg->accept(*this);
        }
    }
};

TEST_F(ASTTest, VisitorPatternTest) {
    auto left = std::make_unique<LiteralExpression>("10", LiteralType::Integer);
    auto right = std::make_unique<LiteralExpression>("20", LiteralType::Integer);
    auto binary = std::make_unique<BinaryExpression>(
        std::move(left), 
        BinaryOperator::Add, 
        std::move(right)
    );
    
    TestVisitor visitor;
    binary->accept(visitor);
    
    // 应该访问3个节点：1个二元表达式 + 2个字面量表达式
    EXPECT_EQ(visitor.visit_count, 3);
}