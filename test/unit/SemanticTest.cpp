/**
 * @file SemanticTest.cpp
 * @brief Starry语言语义分析单元测试
 * @author Starry Team
 * @date 2024
 */

#include <gtest/gtest.h>
#include "starry/semantic/SymbolTable.h"
#include <stdexcept>

using namespace starry::semantic;

class SemanticTest : public ::testing::Test {
protected:
    std::unique_ptr<SymbolTable> symbol_table;
    
    void SetUp() override {
        symbol_table = std::make_unique<SymbolTable>();
    }
    
    void TearDown() override {
        symbol_table.reset();
    }
};

// 测试符号创建
TEST_F(SemanticTest, SymbolCreationTest) {
    Symbol symbol("test_var", SymbolType::Variable, "int");
    
    EXPECT_EQ(symbol.getName(), "test_var");
    EXPECT_EQ(symbol.getType(), SymbolType::Variable);
    EXPECT_EQ(symbol.getDataType(), "int");
    EXPECT_FALSE(symbol.isInitialized());
    
    symbol.setInitialized(true);
    EXPECT_TRUE(symbol.isInitialized());
}

// 测试符号表基本操作
TEST_F(SemanticTest, SymbolTableBasicTest) {
    // 添加符号
    symbol_table->addSymbol("x", SymbolType::Variable, "int");
    symbol_table->addSymbol("func", SymbolType::Function, "void");
    
    // 查找符号
    Symbol* x_symbol = symbol_table->findSymbol("x");
    ASSERT_NE(x_symbol, nullptr);
    EXPECT_EQ(x_symbol->getName(), "x");
    EXPECT_EQ(x_symbol->getType(), SymbolType::Variable);
    EXPECT_EQ(x_symbol->getDataType(), "int");
    
    Symbol* func_symbol = symbol_table->findSymbol("func");
    ASSERT_NE(func_symbol, nullptr);
    EXPECT_EQ(func_symbol->getName(), "func");
    EXPECT_EQ(func_symbol->getType(), SymbolType::Function);
    
    // 检查符号是否定义
    EXPECT_TRUE(symbol_table->isSymbolDefined("x"));
    EXPECT_TRUE(symbol_table->isSymbolDefined("func"));
    EXPECT_FALSE(symbol_table->isSymbolDefined("undefined_var"));
}

// 测试重复定义错误
TEST_F(SemanticTest, DuplicateDefinitionTest) {
    symbol_table->addSymbol("duplicate", SymbolType::Variable, "int");
    
    // 尝试重复定义应该抛出异常
    EXPECT_THROW(
        symbol_table->addSymbol("duplicate", SymbolType::Variable, "double"),
        std::runtime_error
    );
}

// 测试作用域管理
TEST_F(SemanticTest, ScopeManagementTest) {
    // 全局作用域
    symbol_table->addSymbol("global_var", SymbolType::Variable, "int");
    EXPECT_TRUE(symbol_table->isSymbolDefined("global_var"));
    
    // 进入新作用域
    symbol_table->enterScope();
    
    // 在新作用域中添加符号
    symbol_table->addSymbol("local_var", SymbolType::Variable, "double");
    EXPECT_TRUE(symbol_table->isSymbolDefined("local_var"));
    EXPECT_TRUE(symbol_table->isSymbolDefined("global_var")); // 仍能访问全局变量
    
    // 在新作用域中可以重新定义全局变量名
    symbol_table->addSymbol("global_var", SymbolType::Variable, "string");
    Symbol* redefined = symbol_table->findSymbol("global_var");
    EXPECT_EQ(redefined->getDataType(), "string"); // 应该找到局部定义
    
    // 退出作用域
    symbol_table->exitScope();
    
    // 局部变量不再可见
    EXPECT_FALSE(symbol_table->isSymbolDefined("local_var"));
    
    // 全局变量恢复原来的定义
    Symbol* global = symbol_table->findSymbol("global_var");
    EXPECT_EQ(global->getDataType(), "int");
}

// 测试嵌套作用域
TEST_F(SemanticTest, NestedScopeTest) {
    // 全局作用域
    symbol_table->addSymbol("var", SymbolType::Variable, "int");
    
    // 第一层嵌套
    symbol_table->enterScope();
    symbol_table->addSymbol("var", SymbolType::Variable, "double");
    
    // 第二层嵌套
    symbol_table->enterScope();
    symbol_table->addSymbol("var", SymbolType::Variable, "string");
    
    // 应该找到最内层的定义
    Symbol* innermost = symbol_table->findSymbol("var");
    EXPECT_EQ(innermost->getDataType(), "string");
    
    // 退出一层
    symbol_table->exitScope();
    Symbol* middle = symbol_table->findSymbol("var");
    EXPECT_EQ(middle->getDataType(), "double");
    
    // 再退出一层
    symbol_table->exitScope();
    Symbol* global = symbol_table->findSymbol("var");
    EXPECT_EQ(global->getDataType(), "int");
}

// 测试不同符号类型
TEST_F(SemanticTest, SymbolTypesTest) {
    symbol_table->addSymbol("var", SymbolType::Variable, "int");
    symbol_table->addSymbol("func", SymbolType::Function, "void");
    symbol_table->addSymbol("param", SymbolType::Parameter, "double");
    
    EXPECT_EQ(symbol_table->findSymbol("var")->getType(), SymbolType::Variable);
    EXPECT_EQ(symbol_table->findSymbol("func")->getType(), SymbolType::Function);
    EXPECT_EQ(symbol_table->findSymbol("param")->getType(), SymbolType::Parameter);
}

// 测试符号初始化状态
TEST_F(SemanticTest, SymbolInitializationTest) {
    symbol_table->addSymbol("uninitialized", SymbolType::Variable, "int");
    symbol_table->addSymbol("initialized", SymbolType::Variable, "int");
    
    Symbol* uninit = symbol_table->findSymbol("uninitialized");
    Symbol* init = symbol_table->findSymbol("initialized");
    
    EXPECT_FALSE(uninit->isInitialized());
    EXPECT_FALSE(init->isInitialized());
    
    // 标记为已初始化
    init->setInitialized(true);
    
    EXPECT_FALSE(uninit->isInitialized());
    EXPECT_TRUE(init->isInitialized());
}