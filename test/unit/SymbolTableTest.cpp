#include <gtest/gtest.h>
#include "starry/semantic/SymbolTable.h"
#include <chrono>

using namespace starry::semantic;

class SymbolTableTest : public ::testing::Test {
protected:
    void SetUp() override {
        symbolTable = std::make_unique<SymbolTable>();
    }
    
    void TearDown() override {
        symbolTable.reset();
    }
    
    std::unique_ptr<SymbolTable> symbolTable;
};

// 测试符号表的基本功能
TEST_F(SymbolTableTest, BasicSymbolOperations) {
    // 测试添加符号
    EXPECT_TRUE(symbolTable->addSymbol("x", "int"));
    EXPECT_TRUE(symbolTable->addSymbol("y", "float"));
    EXPECT_TRUE(symbolTable->addSymbol("name", "string"));
    
    // 测试查找符号
    EXPECT_EQ(symbolTable->getSymbolType("x"), "int");
    EXPECT_EQ(symbolTable->getSymbolType("y"), "float");
    EXPECT_EQ(symbolTable->getSymbolType("name"), "string");
    
    // 测试不存在的符号
    EXPECT_EQ(symbolTable->getSymbolType("unknown"), "");
    
    // 测试符号是否存在
    EXPECT_TRUE(symbolTable->hasSymbol("x"));
    EXPECT_TRUE(symbolTable->hasSymbol("y"));
    EXPECT_FALSE(symbolTable->hasSymbol("unknown"));
}

// 测试重复符号添加
TEST_F(SymbolTableTest, DuplicateSymbols) {
    // 第一次添加应该成功
    EXPECT_TRUE(symbolTable->addSymbol("x", "int"));
    
    // 重复添加相同符号应该失败
    EXPECT_FALSE(symbolTable->addSymbol("x", "float"));
    
    // 原始类型应该保持不变
    EXPECT_EQ(symbolTable->getSymbolType("x"), "int");
}

// 测试作用域管理
TEST_F(SymbolTableTest, ScopeManagement) {
    // 在全局作用域添加符号
    EXPECT_TRUE(symbolTable->addSymbol("global_var", "int"));
    
    // 进入新作用域
    symbolTable->enterScope();
    
    // 在新作用域中添加符号
    EXPECT_TRUE(symbolTable->addSymbol("local_var", "float"));
    
    // 应该能找到局部和全局符号
    EXPECT_TRUE(symbolTable->hasSymbol("local_var"));
    EXPECT_TRUE(symbolTable->hasSymbol("global_var"));
    
    // 在局部作用域中可以重新定义全局符号
    EXPECT_TRUE(symbolTable->addSymbol("global_var", "string"));
    EXPECT_EQ(symbolTable->getSymbolType("global_var"), "string");
    
    // 退出作用域
    symbolTable->exitScope();
    
    // 局部符号应该不可见
    EXPECT_FALSE(symbolTable->hasSymbol("local_var"));
    
    // 全局符号应该恢复原始类型
    EXPECT_EQ(symbolTable->getSymbolType("global_var"), "int");
}

// 测试嵌套作用域
TEST_F(SymbolTableTest, NestedScopes) {
    // 全局作用域
    symbolTable->addSymbol("x", "int");
    
    // 第一层嵌套
    symbolTable->enterScope();
    symbolTable->addSymbol("y", "float");
    
    // 第二层嵌套
    symbolTable->enterScope();
    symbolTable->addSymbol("z", "string");
    
    // 应该能访问所有层级的符号
    EXPECT_TRUE(symbolTable->hasSymbol("x"));
    EXPECT_TRUE(symbolTable->hasSymbol("y"));
    EXPECT_TRUE(symbolTable->hasSymbol("z"));
    
    // 退出一层
    symbolTable->exitScope();
    EXPECT_TRUE(symbolTable->hasSymbol("x"));
    EXPECT_TRUE(symbolTable->hasSymbol("y"));
    EXPECT_FALSE(symbolTable->hasSymbol("z"));
    
    // 退出到全局
    symbolTable->exitScope();
    EXPECT_TRUE(symbolTable->hasSymbol("x"));
    EXPECT_FALSE(symbolTable->hasSymbol("y"));
    EXPECT_FALSE(symbolTable->hasSymbol("z"));
}

// 测试符号删除
TEST_F(SymbolTableTest, SymbolRemoval) {
    // 添加符号
    symbolTable->addSymbol("temp", "int");
    EXPECT_TRUE(symbolTable->hasSymbol("temp"));
    
    // 删除符号
    EXPECT_TRUE(symbolTable->removeSymbol("temp"));
    EXPECT_FALSE(symbolTable->hasSymbol("temp"));
    
    // 删除不存在的符号应该返回false
    EXPECT_FALSE(symbolTable->removeSymbol("nonexistent"));
}

// 测试符号表清空
TEST_F(SymbolTableTest, ClearSymbolTable) {
    // 添加一些符号
    symbolTable->addSymbol("a", "int");
    symbolTable->addSymbol("b", "float");
    symbolTable->addSymbol("c", "string");
    
    EXPECT_TRUE(symbolTable->hasSymbol("a"));
    EXPECT_TRUE(symbolTable->hasSymbol("b"));
    EXPECT_TRUE(symbolTable->hasSymbol("c"));
    
    // 清空符号表
    symbolTable->clear();
    
    EXPECT_FALSE(symbolTable->hasSymbol("a"));
    EXPECT_FALSE(symbolTable->hasSymbol("b"));
    EXPECT_FALSE(symbolTable->hasSymbol("c"));
}

// 测试获取所有符号
TEST_F(SymbolTableTest, GetAllSymbols) {
    // 添加符号
    symbolTable->addSymbol("x", "int");
    symbolTable->addSymbol("y", "float");
    symbolTable->addSymbol("z", "string");
    
    // 获取所有符号
    auto symbols = symbolTable->getAllSymbols();
    
    EXPECT_EQ(symbols.size(), 3);
    EXPECT_NE(symbols.find("x"), symbols.end());
    EXPECT_NE(symbols.find("y"), symbols.end());
    EXPECT_NE(symbols.find("z"), symbols.end());
    
    EXPECT_EQ(symbols["x"], "int");
    EXPECT_EQ(symbols["y"], "float");
    EXPECT_EQ(symbols["z"], "string");
}

// 测试符号表大小
TEST_F(SymbolTableTest, SymbolTableSize) {
    EXPECT_EQ(symbolTable->size(), 0);
    
    symbolTable->addSymbol("a", "int");
    EXPECT_EQ(symbolTable->size(), 1);
    
    symbolTable->addSymbol("b", "float");
    EXPECT_EQ(symbolTable->size(), 2);
    
    symbolTable->removeSymbol("a");
    EXPECT_EQ(symbolTable->size(), 1);
    
    symbolTable->clear();
    EXPECT_EQ(symbolTable->size(), 0);
}

// 测试符号表是否为空
TEST_F(SymbolTableTest, IsEmpty) {
    EXPECT_TRUE(symbolTable->isEmpty());
    
    symbolTable->addSymbol("test", "int");
    EXPECT_FALSE(symbolTable->isEmpty());
    
    symbolTable->clear();
    EXPECT_TRUE(symbolTable->isEmpty());
}

// 测试函数符号
TEST_F(SymbolTableTest, FunctionSymbols) {
    // 添加函数符号
    EXPECT_TRUE(symbolTable->addFunction("add", "int(int,int)"));
    EXPECT_TRUE(symbolTable->addFunction("print", "void(string)"));
    
    // 查找函数
    EXPECT_EQ(symbolTable->getFunctionSignature("add"), "int(int,int)");
    EXPECT_EQ(symbolTable->getFunctionSignature("print"), "void(string)");
    
    // 检查函数是否存在
    EXPECT_TRUE(symbolTable->hasFunction("add"));
    EXPECT_TRUE(symbolTable->hasFunction("print"));
    EXPECT_FALSE(symbolTable->hasFunction("unknown"));
}

// 测试类型符号
TEST_F(SymbolTableTest, TypeSymbols) {
    // 添加类型符号
    EXPECT_TRUE(symbolTable->addType("Point", "struct"));
    EXPECT_TRUE(symbolTable->addType("Vector", "class"));
    
    // 查找类型
    EXPECT_EQ(symbolTable->getTypeKind("Point"), "struct");
    EXPECT_EQ(symbolTable->getTypeKind("Vector"), "class");
    
    // 检查类型是否存在
    EXPECT_TRUE(symbolTable->hasType("Point"));
    EXPECT_TRUE(symbolTable->hasType("Vector"));
    EXPECT_FALSE(symbolTable->hasType("unknown"));
}

// 测试符号表的打印功能
TEST_F(SymbolTableTest, PrintSymbolTable) {
    symbolTable->addSymbol("x", "int");
    symbolTable->addSymbol("y", "float");
    symbolTable->addFunction("test", "void()");
    symbolTable->addType("MyClass", "class");
    
    // 这个测试主要确保打印功能不会崩溃
    // 实际输出需要手动验证
    EXPECT_NO_THROW(symbolTable->print());
}

// 测试符号表的复制构造
TEST_F(SymbolTableTest, CopyConstruction) {
    // 添加一些符号
    symbolTable->addSymbol("a", "int");
    symbolTable->addSymbol("b", "float");
    
    // 创建副本
    SymbolTable copy(*symbolTable);
    
    // 验证副本包含相同的符号
    EXPECT_TRUE(copy.hasSymbol("a"));
    EXPECT_TRUE(copy.hasSymbol("b"));
    EXPECT_EQ(copy.getSymbolType("a"), "int");
    EXPECT_EQ(copy.getSymbolType("b"), "float");
    
    // 修改原表不应影响副本
    symbolTable->addSymbol("c", "string");
    EXPECT_FALSE(copy.hasSymbol("c"));
}

// 测试符号表的赋值操作
TEST_F(SymbolTableTest, AssignmentOperator) {
    // 创建另一个符号表
    SymbolTable other;
    other.addSymbol("x", "int");
    other.addSymbol("y", "float");
    
    // 赋值操作
    *symbolTable = other;
    
    // 验证赋值结果
    EXPECT_TRUE(symbolTable->hasSymbol("x"));
    EXPECT_TRUE(symbolTable->hasSymbol("y"));
    EXPECT_EQ(symbolTable->getSymbolType("x"), "int");
    EXPECT_EQ(symbolTable->getSymbolType("y"), "float");
}

// 性能测试：大量符号操作
TEST_F(SymbolTableTest, PerformanceTest) {
    const int NUM_SYMBOLS = 10000;
    
    // 添加大量符号
    auto start = std::chrono::high_resolution_clock::now();
    for (int i = 0; i < NUM_SYMBOLS; ++i) {
        std::string name = "var" + std::to_string(i);
        symbolTable->addSymbol(name, "int");
    }
    auto end = std::chrono::high_resolution_clock::now();
    
    auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(end - start);
    std::cout << "添加 " << NUM_SYMBOLS << " 个符号耗时: " << duration.count() << "ms" << std::endl;
    
    // 验证符号数量
    EXPECT_EQ(symbolTable->size(), NUM_SYMBOLS);
    
    // 查找符号性能测试
    start = std::chrono::high_resolution_clock::now();
    for (int i = 0; i < NUM_SYMBOLS; ++i) {
        std::string name = "var" + std::to_string(i);
        EXPECT_TRUE(symbolTable->hasSymbol(name));
    }
    end = std::chrono::high_resolution_clock::now();
    
    duration = std::chrono::duration_cast<std::chrono::milliseconds>(end - start);
    std::cout << "查找 " << NUM_SYMBOLS << " 个符号耗时: " << duration.count() << "ms" << std::endl;
}