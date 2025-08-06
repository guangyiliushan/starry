#include <gtest/gtest.h>
#include "starry/semantic/TypeChecker.h"
#include "starry/AST.h"
#include <memory>

using namespace starry::semantic;
using namespace starry::ast;

class TypeCheckerTest : public ::testing::Test {
protected:
    void SetUp() override {
        typeChecker = std::make_unique<TypeChecker>();
    }
    
    void TearDown() override {
        typeChecker.reset();
    }
    
    std::unique_ptr<TypeChecker> typeChecker;
};

TEST_F(TypeCheckerTest, CheckLiteralExpressions) {
    // 测试整数字面量
    auto intLiteral = std::make_unique<LiteralExpression>("42", LiteralType::INTEGER);
    auto intType = typeChecker->checkExpression(intLiteral.get());
    ASSERT_NE(intType, nullptr);
    EXPECT_EQ(intType->getKind(), Type::Kind::INTEGER);
    
    // 测试浮点数字面量
    auto floatLiteral = std::make_unique<LiteralExpression>("3.14", LiteralType::FLOAT);
    auto floatType = typeChecker->checkExpression(floatLiteral.get());
    ASSERT_NE(floatType, nullptr);
    EXPECT_EQ(floatType->getKind(), Type::Kind::FLOAT);
    
    // 测试字符串字面量
    auto stringLiteral = std::make_unique<LiteralExpression>("hello", LiteralType::STRING);
    auto stringType = typeChecker->checkExpression(stringLiteral.get());
    ASSERT_NE(stringType, nullptr);
    EXPECT_EQ(stringType->getKind(), Type::Kind::STRING);
    
    // 测试布尔字面量
    auto boolLiteral = std::make_unique<LiteralExpression>("true", LiteralType::BOOLEAN);
    auto boolType = typeChecker->checkExpression(boolLiteral.get());
    ASSERT_NE(boolType, nullptr);
    EXPECT_EQ(boolType->getKind(), Type::Kind::BOOLEAN);
}

TEST_F(TypeCheckerTest, CheckBinaryExpressions) {
    // 测试算术表达式
    auto left = std::make_unique<LiteralExpression>("10", LiteralType::INTEGER);
    auto right = std::make_unique<LiteralExpression>("20", LiteralType::INTEGER);
    auto binaryExpr = std::make_unique<BinaryExpression>(
        std::move(left), BinaryOperator::ADD, std::move(right));
    
    auto resultType = typeChecker->checkExpression(binaryExpr.get());
    ASSERT_NE(resultType, nullptr);
    EXPECT_EQ(resultType->getKind(), Type::Kind::INTEGER);
}

TEST_F(TypeCheckerTest, CheckVariableDeclaration) {
    // 测试变量声明
    auto initializer = std::make_unique<LiteralExpression>("42", LiteralType::INTEGER);
    auto varDecl = std::make_unique<VariableDeclaration>("x", std::move(initializer));
    
    EXPECT_NO_THROW(typeChecker->checkStatement(varDecl.get()));
}

TEST_F(TypeCheckerTest, CheckTypeCompatibility) {
    // 测试类型兼容性检查
    auto intLiteral = std::make_unique<LiteralExpression>("42", LiteralType::INTEGER);
    auto floatLiteral = std::make_unique<LiteralExpression>("3.14", LiteralType::FLOAT);
    
    auto intType = typeChecker->checkExpression(intLiteral.get());
    auto floatType = typeChecker->checkExpression(floatLiteral.get());
    
    // int可以隐式转换为float
    EXPECT_TRUE(typeChecker->isTypeCompatible(floatType, intType));
    // float不能隐式转换为int
    EXPECT_FALSE(typeChecker->isTypeCompatible(intType, floatType));
}

TEST_F(TypeCheckerTest, CheckUndefinedVariable) {
    // 测试未定义变量的错误处理
    auto identifier = std::make_unique<IdentifierExpression>("undefinedVar");
    
    EXPECT_THROW(typeChecker->checkExpression(identifier.get()), std::runtime_error);
}

TEST_F(TypeCheckerTest, CheckIfStatement) {
    // 测试if语句类型检查
    auto condition = std::make_unique<LiteralExpression>("true", LiteralType::BOOLEAN);
    auto thenStmt = std::make_unique<ExpressionStatement>(
        std::make_unique<LiteralExpression>("1", LiteralType::INTEGER));
    
    auto ifStmt = std::make_unique<IfStatement>(
        std::move(condition), std::move(thenStmt), nullptr);
    
    EXPECT_NO_THROW(typeChecker->checkStatement(ifStmt.get()));
}

TEST_F(TypeCheckerTest, CheckInvalidIfCondition) {
    // 测试if语句条件类型错误
    auto condition = std::make_unique<LiteralExpression>("42", LiteralType::INTEGER);
    auto thenStmt = std::make_unique<ExpressionStatement>(
        std::make_unique<LiteralExpression>("1", LiteralType::INTEGER));
    
    auto ifStmt = std::make_unique<IfStatement>(
        std::move(condition), std::move(thenStmt), nullptr);
    
    EXPECT_THROW(typeChecker->checkStatement(ifStmt.get()), std::runtime_error);
}

TEST_F(TypeCheckerTest, CheckFunctionDeclaration) {
    // 测试函数声明
    std::vector<std::string> params = {"x", "y"};
    auto body = std::make_unique<BlockStatement>(std::vector<std::unique_ptr<Statement>>());
    
    auto funcDecl = std::make_unique<FunctionDeclaration>("add", std::move(params), std::move(body));
    
    EXPECT_NO_THROW(typeChecker->checkStatement(funcDecl.get()));
}

TEST_F(TypeCheckerTest, CheckDuplicateVariableDeclaration) {
    // 测试重复变量声明
    auto init1 = std::make_unique<LiteralExpression>("1", LiteralType::INTEGER);
    auto init2 = std::make_unique<LiteralExpression>("2", LiteralType::INTEGER);
    
    auto varDecl1 = std::make_unique<VariableDeclaration>("x", std::move(init1));
    auto varDecl2 = std::make_unique<VariableDeclaration>("x", std::move(init2));
    
    EXPECT_NO_THROW(typeChecker->checkStatement(varDecl1.get()));
    EXPECT_THROW(typeChecker->checkStatement(varDecl2.get()), std::runtime_error);
}

TEST_F(TypeCheckerTest, CheckLogicalOperations) {
    // 测试逻辑操作
    auto left = std::make_unique<LiteralExpression>("true", LiteralType::BOOLEAN);
    auto right = std::make_unique<LiteralExpression>("false", LiteralType::BOOLEAN);
    
    auto logicalAnd = std::make_unique<BinaryExpression>(
        std::move(left), BinaryOperator::LOGICAL_AND, std::move(right));
    
    auto resultType = typeChecker->checkExpression(logicalAnd.get());
    ASSERT_NE(resultType, nullptr);
    EXPECT_EQ(resultType->getKind(), Type::Kind::BOOLEAN);
}

TEST_F(TypeCheckerTest, CheckUnaryOperations) {
    // 测试一元操作
    auto operand = std::make_unique<LiteralExpression>("42", LiteralType::INTEGER);
    auto unaryExpr = std::make_unique<UnaryExpression>(UnaryOperator::MINUS, std::move(operand));
    
    auto resultType = typeChecker->checkExpression(unaryExpr.get());
    ASSERT_NE(resultType, nullptr);
    EXPECT_EQ(resultType->getKind(), Type::Kind::INTEGER);
}

TEST_F(TypeCheckerTest, CheckInvalidUnaryOperation) {
    // 测试无效的一元操作
    auto operand = std::make_unique<LiteralExpression>("hello", LiteralType::STRING);
    auto unaryExpr = std::make_unique<UnaryExpression>(UnaryOperator::MINUS, std::move(operand));
    
    EXPECT_THROW(typeChecker->checkExpression(unaryExpr.get()), std::runtime_error);
}