#include <gtest/gtest.h>
#include "starry/Parser.h"
#include "starry/Lexer.h"
#include <sstream>
#include <memory>

using namespace starry;

class ParserTest : public ::testing::Test {
protected:
    std::unique_ptr<Parser> createParser(const std::string& source) {
        auto stream = std::make_unique<std::istringstream>(source);
        auto lexer = std::make_unique<Lexer>(std::move(stream));
        return std::make_unique<Parser>(std::move(lexer));
    }
};

TEST_F(ParserTest, ParseEmptyProgram) {
    auto parser = createParser("");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 0);
    EXPECT_FALSE(parser->hasErrors());
}

TEST_F(ParserTest, ParseVariableDeclaration) {
    auto parser = createParser("var x: int = 42;");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto varDecl = dynamic_cast<VariableDeclarationNode*>(stmt);
    ASSERT_NE(varDecl, nullptr);
    EXPECT_EQ(varDecl->getName(), "x");
    EXPECT_EQ(varDecl->getType(), "int");
    EXPECT_FALSE(varDecl->isConst());
    EXPECT_NE(varDecl->getInitializer(), nullptr);
}

TEST_F(ParserTest, ParseConstDeclaration) {
    auto parser = createParser("const PI: double = 3.14159;");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto varDecl = dynamic_cast<VariableDeclarationNode*>(stmt);
    ASSERT_NE(varDecl, nullptr);
    EXPECT_EQ(varDecl->getName(), "PI");
    EXPECT_EQ(varDecl->getType(), "double");
    EXPECT_TRUE(varDecl->isConst());
}

TEST_F(ParserTest, ParseFunctionDeclaration) {
    auto parser = createParser(R"(
        function add(a: int, b: int) -> int {
            return a + b;
        }
    )");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto funcDecl = dynamic_cast<FunctionDeclarationNode*>(stmt);
    ASSERT_NE(funcDecl, nullptr);
    EXPECT_EQ(funcDecl->getName(), "add");
    EXPECT_EQ(funcDecl->getReturnType(), "int");
    EXPECT_EQ(funcDecl->getParameters().size(), 2);
}

TEST_F(ParserTest, ParseClassDeclaration) {
    auto parser = createParser(R"(
        class Point {
            var x: int;
            var y: int;
            
            function constructor(x: int, y: int) {
                this.x = x;
                this.y = y;
            }
        }
    )");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto classDecl = dynamic_cast<ClassDeclarationNode*>(stmt);
    ASSERT_NE(classDecl, nullptr);
    EXPECT_EQ(classDecl->getName(), "Point");
    EXPECT_EQ(classDecl->getSuperclass(), "");
    EXPECT_EQ(classDecl->getMembers().size(), 3);
}

TEST_F(ParserTest, ParseIfStatement) {
    auto parser = createParser(R"(
        if (x > 0) {
            print("positive");
        } else {
            print("non-positive");
        }
    )");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto ifStmt = dynamic_cast<IfStatementNode*>(stmt);
    ASSERT_NE(ifStmt, nullptr);
    EXPECT_NE(ifStmt->getCondition(), nullptr);
    EXPECT_NE(ifStmt->getThenBranch(), nullptr);
    EXPECT_NE(ifStmt->getElseBranch(), nullptr);
}

TEST_F(ParserTest, ParseWhileStatement) {
    auto parser = createParser(R"(
        while (i < 10) {
            i = i + 1;
        }
    )");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto whileStmt = dynamic_cast<WhileStatementNode*>(stmt);
    ASSERT_NE(whileStmt, nullptr);
    EXPECT_NE(whileStmt->getCondition(), nullptr);
    EXPECT_NE(whileStmt->getBody(), nullptr);
}

TEST_F(ParserTest, ParseForStatement) {
    auto parser = createParser(R"(
        for (var i: int = 0; i < 10; i = i + 1) {
            print(i);
        }
    )");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto forStmt = dynamic_cast<ForStatementNode*>(stmt);
    ASSERT_NE(forStmt, nullptr);
    EXPECT_NE(forStmt->getInitializer(), nullptr);
    EXPECT_NE(forStmt->getCondition(), nullptr);
    EXPECT_NE(forStmt->getIncrement(), nullptr);
    EXPECT_NE(forStmt->getBody(), nullptr);
}

TEST_F(ParserTest, ParseBinaryExpression) {
    auto parser = createParser("x + y * z;");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto exprStmt = dynamic_cast<ExpressionStatementNode*>(stmt);
    ASSERT_NE(exprStmt, nullptr);
    
    auto expr = exprStmt->getExpression();
    auto binaryExpr = dynamic_cast<BinaryExpressionNode*>(expr);
    ASSERT_NE(binaryExpr, nullptr);
    EXPECT_EQ(binaryExpr->getOperator(), "+");
}

TEST_F(ParserTest, ParseFunctionCall) {
    auto parser = createParser("print(\"Hello, World!\");");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto exprStmt = dynamic_cast<ExpressionStatementNode*>(stmt);
    ASSERT_NE(exprStmt, nullptr);
    
    auto expr = exprStmt->getExpression();
    auto callExpr = dynamic_cast<CallExpressionNode*>(expr);
    ASSERT_NE(callExpr, nullptr);
    EXPECT_EQ(callExpr->getArguments().size(), 1);
}

TEST_F(ParserTest, ParseMemberAccess) {
    auto parser = createParser("obj.property;");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto exprStmt = dynamic_cast<ExpressionStatementNode*>(stmt);
    ASSERT_NE(exprStmt, nullptr);
    
    auto expr = exprStmt->getExpression();
    auto memberExpr = dynamic_cast<MemberExpressionNode*>(expr);
    ASSERT_NE(memberExpr, nullptr);
    EXPECT_EQ(memberExpr->getProperty(), "property");
}

TEST_F(ParserTest, ParseArrayAccess) {
    auto parser = createParser("arr[0];");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto exprStmt = dynamic_cast<ExpressionStatementNode*>(stmt);
    ASSERT_NE(exprStmt, nullptr);
    
    auto expr = exprStmt->getExpression();
    auto indexExpr = dynamic_cast<IndexExpressionNode*>(expr);
    ASSERT_NE(indexExpr, nullptr);
}

TEST_F(ParserTest, ParseAssignment) {
    auto parser = createParser("x = 42;");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto exprStmt = dynamic_cast<ExpressionStatementNode*>(stmt);
    ASSERT_NE(exprStmt, nullptr);
    
    auto expr = exprStmt->getExpression();
    auto assignExpr = dynamic_cast<AssignmentExpressionNode*>(expr);
    ASSERT_NE(assignExpr, nullptr);
}

TEST_F(ParserTest, ParseComplexExpression) {
    auto parser = createParser("(a + b) * (c - d);");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
}

TEST_F(ParserTest, ParseSyntaxError) {
    auto parser = createParser("var x = ;"); // 缺少初始化值
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_TRUE(parser->hasErrors());
    EXPECT_GT(parser->getErrors().size(), 0);
}

TEST_F(ParserTest, ParseMultipleStatements) {
    auto parser = createParser(R"(
        var x: int = 10;
        var y: int = 20;
        var sum: int = x + y;
        print(sum);
    )");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 4);
    EXPECT_FALSE(parser->hasErrors());
}

TEST_F(ParserTest, ParseNestedBlocks) {
    auto parser = createParser(R"(
        {
            var x: int = 1;
            {
                var y: int = 2;
                print(x + y);
            }
        }
    )");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto blockStmt = dynamic_cast<BlockStatementNode*>(stmt);
    ASSERT_NE(blockStmt, nullptr);
    EXPECT_EQ(blockStmt->getStatements().size(), 2);
}

TEST_F(ParserTest, ParseReturnStatement) {
    auto parser = createParser("return x + y;");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto returnStmt = dynamic_cast<ReturnStatementNode*>(stmt);
    ASSERT_NE(returnStmt, nullptr);
    EXPECT_NE(returnStmt->getValue(), nullptr);
}

TEST_F(ParserTest, ParseEmptyReturn) {
    auto parser = createParser("return;");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto returnStmt = dynamic_cast<ReturnStatementNode*>(stmt);
    ASSERT_NE(returnStmt, nullptr);
    EXPECT_EQ(returnStmt->getValue(), nullptr);
}

TEST_F(ParserTest, ParseBreakContinue) {
    auto parser = createParser(R"(
        while (true) {
            if (condition1) {
                break;
            }
            if (condition2) {
                continue;
            }
        }
    )");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
}

TEST_F(ParserTest, ParseGenericType) {
    auto parser = createParser("var list: Array<int> = createArray();");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto varDecl = dynamic_cast<VariableDeclarationNode*>(stmt);
    ASSERT_NE(varDecl, nullptr);
    EXPECT_EQ(varDecl->getType(), "Array<int>");
}

TEST_F(ParserTest, ParseClassInheritance) {
    auto parser = createParser(R"(
        class Dog extends Animal {
            function bark() {
                print("Woof!");
            }
        }
    )");
    auto program = parser->parseProgram();
    
    ASSERT_NE(program, nullptr);
    EXPECT_EQ(program->getStatements().size(), 1);
    EXPECT_FALSE(parser->hasErrors());
    
    auto stmt = program->getStatements()[0].get();
    auto classDecl = dynamic_cast<ClassDeclarationNode*>(stmt);
    ASSERT_NE(classDecl, nullptr);
    EXPECT_EQ(classDecl->getName(), "Dog");
    EXPECT_EQ(classDecl->getSuperclass(), "Animal");
}
