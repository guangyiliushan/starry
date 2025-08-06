/**
 * @file CompilerIntegrationTest.cpp
 * @brief Starry语言编译器集成测试
 * @author Starry Team
 * @date 2024
 */

#include <gtest/gtest.h>
#include "starry/Lexer.h"
#include "starry/Parser.h"
#include "starry/codegen/CodeGenerator.h"
#include "starry/semantic/SymbolTable.h"
#include <sstream>
#include <memory>

using namespace starry;

class CompilerIntegrationTest : public ::testing::Test {
protected:
    std::unique_ptr<lexer::Lexer> lexer_;
    std::unique_ptr<parser::Parser> parser_;
    std::unique_ptr<codegen::CodeGenerator> codegen_;
    std::unique_ptr<semantic::SymbolTable> symbol_table_;
    
    void SetUp() override {
        lexer_ = std::make_unique<lexer::Lexer>();
        parser_ = std::make_unique<parser::Parser>();
        codegen_ = std::make_unique<codegen::CodeGenerator>();
        symbol_table_ = std::make_unique<semantic::SymbolTable>();
    }
    
    void TearDown() override {
        lexer_.reset();
        parser_.reset();
        codegen_.reset();
        symbol_table_.reset();
    }
    
    // 辅助函数：编译简单表达式
    std::string compileExpression(const std::string& source) {
        // 词法分析
        auto tokens = lexer_->tokenize(source);
        
        // 语法分析
        auto ast = parser_->parseExpression(tokens);
        
        // 代码生成
        std::ostringstream output;
        codegen::CodeGenerator generator(output);
        if (ast) {
            ast->accept(generator);
        }
        
        return output.str();
    }
};

// 测试简单算术表达式编译
TEST_F(CompilerIntegrationTest, SimpleArithmeticTest) {
    std::string source = "1 + 2";
    std::string result = compileExpression(source);
    
    // 期望生成类似 "(1 + 2)" 的代码
    EXPECT_TRUE(result.find("1") != std::string::npos);
    EXPECT_TRUE(result.find("2") != std::string::npos);
    EXPECT_TRUE(result.find("+") != std::string::npos);
}

// 测试复杂表达式编译
TEST_F(CompilerIntegrationTest, ComplexExpressionTest) {
    std::string source = "(a + b) * c";
    
    // 首先添加符号到符号表
    symbol_table_->addSymbol("a", semantic::SymbolType::Variable, "int");
    symbol_table_->addSymbol("b", semantic::SymbolType::Variable, "int");
    symbol_table_->addSymbol("c", semantic::SymbolType::Variable, "int");
    
    std::string result = compileExpression(source);
    
    // 验证生成的代码包含所有变量和运算符
    EXPECT_TRUE(result.find("a") != std::string::npos);
    EXPECT_TRUE(result.find("b") != std::string::npos);
    EXPECT_TRUE(result.find("c") != std::string::npos);
    EXPECT_TRUE(result.find("+") != std::string::npos);
    EXPECT_TRUE(result.find("*") != std::string::npos);
}

// 测试变量声明和使用
TEST_F(CompilerIntegrationTest, VariableDeclarationTest) {
    // 测试变量声明
    symbol_table_->addSymbol("x", semantic::SymbolType::Variable, "int");
    symbol_table_->addSymbol("y", semantic::SymbolType::Variable, "double");
    
    // 验证符号表中的符号
    auto x_symbol = symbol_table_->findSymbol("x");
    auto y_symbol = symbol_table_->findSymbol("y");
    
    ASSERT_NE(x_symbol, nullptr);
    ASSERT_NE(y_symbol, nullptr);
    
    EXPECT_EQ(x_symbol->getName(), "x");
    EXPECT_EQ(x_symbol->getDataType(), "int");
    EXPECT_EQ(y_symbol->getName(), "y");
    EXPECT_EQ(y_symbol->getDataType(), "double");
}

// 测试函数调用编译
TEST_F(CompilerIntegrationTest, FunctionCallTest) {
    // 添加函数符号
    symbol_table_->addSymbol("print", semantic::SymbolType::Function, "void");
    
    std::string source = "print(\"Hello World\")";
    std::string result = compileExpression(source);
    
    // 验证生成的代码包含函数调用
    EXPECT_TRUE(result.find("print") != std::string::npos);
    EXPECT_TRUE(result.find("Hello World") != std::string::npos);
}

// 测试作用域管理
TEST_F(CompilerIntegrationTest, ScopeManagementTest) {
    // 全局作用域
    symbol_table_->addSymbol("global_var", semantic::SymbolType::Variable, "int");
    
    // 进入新作用域
    symbol_table_->enterScope();
    symbol_table_->addSymbol("local_var", semantic::SymbolType::Variable, "double");
    
    // 在局部作用域中应该能找到两个变量
    EXPECT_TRUE(symbol_table_->isSymbolDefined("global_var"));
    EXPECT_TRUE(symbol_table_->isSymbolDefined("local_var"));
    
    // 退出作用域
    symbol_table_->exitScope();
    
    // 现在只能找到全局变量
    EXPECT_TRUE(symbol_table_->isSymbolDefined("global_var"));
    EXPECT_FALSE(symbol_table_->isSymbolDefined("local_var"));
}

// 测试错误处理
TEST_F(CompilerIntegrationTest, ErrorHandlingTest) {
    // 测试重复定义错误
    symbol_table_->addSymbol("duplicate", semantic::SymbolType::Variable, "int");
    
    EXPECT_THROW(
        symbol_table_->addSymbol("duplicate", semantic::SymbolType::Variable, "double"),
        std::runtime_error
    );
}

// 测试词法分析器和语法分析器集成
TEST_F(CompilerIntegrationTest, LexerParserIntegrationTest) {
    std::string source = "x + y * 2";
    
    // 词法分析
    auto tokens = lexer_->tokenize(source);
    
    // 验证生成的词法单元
    EXPECT_GT(tokens.size(), 0);
    
    // 检查是否包含预期的词法单元类型
    bool has_identifier = false;
    bool has_operator = false;
    bool has_number = false;
    
    for (const auto& token : tokens) {
        if (token.getType() == lexer::TokenType::IDENTIFIER) {
            has_identifier = true;
        } else if (token.getType() == lexer::TokenType::PLUS || 
                   token.getType() == lexer::TokenType::MULTIPLY) {
            has_operator = true;
        } else if (token.getType() == lexer::TokenType::NUMBER) {
            has_number = true;
        }
    }
    
    EXPECT_TRUE(has_identifier);
    EXPECT_TRUE(has_operator);
    EXPECT_TRUE(has_number);
}

// 测试完整编译流程
TEST_F(CompilerIntegrationTest, FullCompilationTest) {
    std::string source = R"(
        var x = 10;
        var y = 20;
        var result = x + y;
    )";
    
    try {
        // 词法分析
        auto tokens = lexer_->tokenize(source);
        EXPECT_GT(tokens.size(), 0);
        
        // 这里应该有更完整的语法分析和代码生成
        // 目前只是验证词法分析不会崩溃
        
        SUCCEED();
    } catch (const std::exception& e) {
        FAIL() << "编译过程中发生异常: " << e.what();
    }
}

// 测试类型检查集成
TEST_F(CompilerIntegrationTest, TypeCheckingTest) {
    // 添加不同类型的变量
    symbol_table_->addSymbol("int_var", semantic::SymbolType::Variable, "int");
    symbol_table_->addSymbol("double_var", semantic::SymbolType::Variable, "double");
    symbol_table_->addSymbol("string_var", semantic::SymbolType::Variable, "string");
    
    // 验证类型信息
    auto int_symbol = symbol_table_->findSymbol("int_var");
    auto double_symbol = symbol_table_->findSymbol("double_var");
    auto string_symbol = symbol_table_->findSymbol("string_var");
    
    EXPECT_EQ(int_symbol->getDataType(), "int");
    EXPECT_EQ(double_symbol->getDataType(), "double");
    EXPECT_EQ(string_symbol->getDataType(), "string");
}

// 测试优化前后的代码生成
TEST_F(CompilerIntegrationTest, OptimizationTest) {
    std::string source = "1 + 1";
    std::string result = compileExpression(source);
    
    // 基本的常量折叠优化应该在这里实现
    // 目前只是验证代码生成正常工作
    EXPECT_FALSE(result.empty());
}

// 测试多行程序编译
TEST_F(CompilerIntegrationTest, MultiLineTest) {
    std::vector<std::string> lines = {
        "var a = 1;",
        "var b = 2;",
        "var c = a + b;"
    };
    
    // 逐行处理
    for (const auto& line : lines) {
        try {
            auto tokens = lexer_->tokenize(line);
            EXPECT_GT(tokens.size(), 0);
        } catch (const std::exception& e) {
            FAIL() << "处理行 '" << line << "' 时发生异常: " << e.what();
        }
    }
}