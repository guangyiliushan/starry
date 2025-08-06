#include <gtest/gtest.h>
#include "starry/runtime/Exception.h"
#include <stdexcept>

using namespace starry::runtime;

class ExceptionTest : public ::testing::Test {
protected:
    void SetUp() override {
        // 清除之前的异常处理器
        setExceptionLogging(true);
    }
    
    void TearDown() override {
        // 清理
    }
};

// 测试基础异常类
TEST_F(ExceptionTest, BasicStarryException) {
    StarryException ex("测试异常", 100);
    
    EXPECT_EQ(ex.getMessage(), "测试异常");
    EXPECT_EQ(ex.getErrorCode(), 100);
    EXPECT_NE(std::string(ex.what()).find("测试异常"), std::string::npos);
    EXPECT_NE(std::string(ex.what()).find("100"), std::string::npos);
}

// 测试运行时异常
TEST_F(ExceptionTest, RuntimeException) {
    RuntimeException ex("运行时错误", 200);
    
    EXPECT_EQ(ex.getErrorCode(), 200);
    EXPECT_NE(std::string(ex.what()).find("运行时错误"), std::string::npos);
}

// 测试内存异常
TEST_F(ExceptionTest, MemoryException) {
    MemoryException ex("内存不足", 300);
    
    EXPECT_EQ(ex.getErrorCode(), 300);
    EXPECT_NE(std::string(ex.what()).find("内存错误"), std::string::npos);
    EXPECT_NE(std::string(ex.what()).find("内存不足"), std::string::npos);
}

// 测试类型异常
TEST_F(ExceptionTest, TypeException) {
    TypeException ex("类型不匹配", 400);
    
    EXPECT_EQ(ex.getErrorCode(), 400);
    EXPECT_NE(std::string(ex.what()).find("类型错误"), std::string::npos);
    EXPECT_NE(std::string(ex.what()).find("类型不匹配"), std::string::npos);
}

// 测试索引异常
TEST_F(ExceptionTest, IndexException) {
    IndexException ex("数组越界", 500);
    
    EXPECT_EQ(ex.getErrorCode(), 500);
    EXPECT_NE(std::string(ex.what()).find("索引错误"), std::string::npos);
    EXPECT_NE(std::string(ex.what()).find("数组越界"), std::string::npos);
}

// 测试空指针异常
TEST_F(ExceptionTest, NullPointerException) {
    NullPointerException ex("访问空指针", 600);
    
    EXPECT_EQ(ex.getErrorCode(), 600);
    EXPECT_NE(std::string(ex.what()).find("空指针错误"), std::string::npos);
    EXPECT_NE(std::string(ex.what()).find("访问空指针"), std::string::npos);
}

// 测试除零异常
TEST_F(ExceptionTest, DivisionByZeroException) {
    DivisionByZeroException ex("除数为零", 700);
    
    EXPECT_EQ(ex.getErrorCode(), 700);
    EXPECT_NE(std::string(ex.what()).find("除零错误"), std::string::npos);
    EXPECT_NE(std::string(ex.what()).find("除数为零"), std::string::npos);
}

// 测试文件异常
TEST_F(ExceptionTest, FileException) {
    FileException ex("文件不存在", 800);
    
    EXPECT_EQ(ex.getErrorCode(), 800);
    EXPECT_NE(std::string(ex.what()).find("文件错误"), std::string::npos);
    EXPECT_NE(std::string(ex.what()).find("文件不存在"), std::string::npos);
}

// 测试网络异常
TEST_F(ExceptionTest, NetworkException) {
    NetworkException ex("连接超时", 900);
    
    EXPECT_EQ(ex.getErrorCode(), 900);
    EXPECT_NE(std::string(ex.what()).find("网络错误"), std::string::npos);
    EXPECT_NE(std::string(ex.what()).find("连接超时"), std::string::npos);
}

// 测试线程异常
TEST_F(ExceptionTest, ThreadException) {
    ThreadException ex("线程死锁", 1000);
    
    EXPECT_EQ(ex.getErrorCode(), 1000);
    EXPECT_NE(std::string(ex.what()).find("线程错误"), std::string::npos);
    EXPECT_NE(std::string(ex.what()).find("线程死锁"), std::string::npos);
}

// 测试异常工厂函数
TEST_F(ExceptionTest, ExceptionFactory) {
    auto ex1 = createException(ExceptionType::RUNTIME, "运行时错误", 100);
    EXPECT_NE(dynamic_cast<RuntimeException*>(ex1.get()), nullptr);
    
    auto ex2 = createException(ExceptionType::MEMORY, "内存错误", 200);
    EXPECT_NE(dynamic_cast<MemoryException*>(ex2.get()), nullptr);
    
    auto ex3 = createException(ExceptionType::TYPE, "类型错误", 300);
    EXPECT_NE(dynamic_cast<TypeException*>(ex3.get()), nullptr);
    
    auto ex4 = createException(ExceptionType::INDEX, "索引错误", 400);
    EXPECT_NE(dynamic_cast<IndexException*>(ex4.get()), nullptr);
    
    auto ex5 = createException(ExceptionType::NULL_POINTER, "空指针错误", 500);
    EXPECT_NE(dynamic_cast<NullPointerException*>(ex5.get()), nullptr);
}

// 测试便捷抛出函数
TEST_F(ExceptionTest, ConvenienceThrowFunctions) {
    EXPECT_THROW(throwRuntimeError("测试", 1), RuntimeException);
    EXPECT_THROW(throwMemoryError("测试", 2), MemoryException);
    EXPECT_THROW(throwTypeError("测试", 3), TypeException);
    EXPECT_THROW(throwIndexError("测试", 4), IndexException);
    EXPECT_THROW(throwNullPointerError("测试", 5), NullPointerException);
    EXPECT_THROW(throwDivisionByZeroError("测试", 6), DivisionByZeroException);
    EXPECT_THROW(throwFileError("测试", 7), FileException);
    EXPECT_THROW(throwNetworkError("测试", 8), NetworkException);
    EXPECT_THROW(throwThreadError("测试", 9), ThreadException);
}

// 测试异常处理器注册
TEST_F(ExceptionTest, ExceptionHandlerRegistration) {
    bool handlerCalled = false;
    std::string caughtMessage;
    int caughtCode = 0;
    
    // 注册异常处理器
    registerExceptionHandler([&](const StarryException& ex) {
        handlerCalled = true;
        caughtMessage = ex.getMessage();
        caughtCode = ex.getErrorCode();
    });
    
    // 创建并处理异常
    StarryException ex("测试异常处理器", 999);
    handleException(ex);
    
    // 验证处理器被调用
    EXPECT_TRUE(handlerCalled);
    EXPECT_EQ(caughtMessage, "测试异常处理器");
    EXPECT_EQ(caughtCode, 999);
}

// 测试多个异常处理器
TEST_F(ExceptionTest, MultipleExceptionHandlers) {
    int handler1Called = 0;
    int handler2Called = 0;
    
    // 注册多个处理器
    registerExceptionHandler([&](const StarryException& ex) {
        handler1Called++;
    });
    
    registerExceptionHandler([&](const StarryException& ex) {
        handler2Called++;
    });
    
    // 处理异常
    StarryException ex("测试多处理器", 888);
    handleException(ex);
    
    // 验证所有处理器都被调用
    EXPECT_EQ(handler1Called, 1);
    EXPECT_EQ(handler2Called, 1);
}

// 测试异常日志开关
TEST_F(ExceptionTest, ExceptionLoggingToggle) {
    // 禁用日志
    setExceptionLogging(false);
    
    // 这个测试主要确保禁用日志不会导致崩溃
    StarryException ex("测试日志禁用", 777);
    EXPECT_NO_THROW(handleException(ex));
    
    // 重新启用日志
    setExceptionLogging(true);
    EXPECT_NO_THROW(handleException(ex));
}

// 测试异常安全执行
TEST_F(ExceptionTest, ExceptionSafeExecution) {
    // 测试正常执行
    auto result1 = executeWithExceptionHandling([]() {
        return 42;
    });
    EXPECT_EQ(result1, 42);
    
    // 测试Starry异常
    EXPECT_THROW(executeWithExceptionHandling([]() {
        throw RuntimeException("测试异常", 123);
        return 0;
    }), StarryException);
    
    // 测试标准异常转换
    EXPECT_THROW(executeWithExceptionHandling([]() {
        throw std::runtime_error("标准异常");
        return 0;
    }), StarryException);
    
    // 测试未知异常转换
    EXPECT_THROW(executeWithExceptionHandling([]() {
        throw 42; // 抛出非异常对象
        return 0;
    }), StarryException);
}

// 测试异常继承关系
TEST_F(ExceptionTest, ExceptionInheritance) {
    RuntimeException runtimeEx("运行时异常", 100);
    MemoryException memoryEx("内存异常", 200);
    
    // 测试多态性
    StarryException* basePtr1 = &runtimeEx;
    StarryException* basePtr2 = &memoryEx;
    
    EXPECT_EQ(basePtr1->getErrorCode(), 100);
    EXPECT_EQ(basePtr2->getErrorCode(), 200);
    
    // 测试dynamic_cast
    EXPECT_NE(dynamic_cast<RuntimeException*>(basePtr1), nullptr);
    EXPECT_EQ(dynamic_cast<MemoryException*>(basePtr1), nullptr);
    
    EXPECT_NE(dynamic_cast<MemoryException*>(basePtr2), nullptr);
    EXPECT_NE(dynamic_cast<RuntimeException*>(basePtr2), nullptr); // MemoryException继承自RuntimeException
}

// 测试异常消息格式
TEST_F(ExceptionTest, ExceptionMessageFormat) {
    StarryException ex("测试消息", 12345);
    std::string whatMsg = ex.what();
    
    // 验证消息格式包含必要信息
    EXPECT_NE(whatMsg.find("Starry异常"), std::string::npos);
    EXPECT_NE(whatMsg.find("12345"), std::string::npos);
    EXPECT_NE(whatMsg.find("测试消息"), std::string::npos);
}

// 性能测试：异常创建和处理
TEST_F(ExceptionTest, ExceptionPerformanceTest) {
    const int NUM_EXCEPTIONS = 1000;
    
    auto start = std::chrono::high_resolution_clock::now();
    
    for (int i = 0; i < NUM_EXCEPTIONS; ++i) {
        try {
            throw RuntimeException("性能测试异常", i);
        } catch (const StarryException& ex) {
            // 捕获并处理异常
            EXPECT_EQ(ex.getErrorCode(), i);
        }
    }
    
    auto end = std::chrono::high_resolution_clock::now();
    auto duration = std::chrono::duration_cast<std::chrono::microseconds>(end - start);
    
    std::cout << "创建和处理 " << NUM_EXCEPTIONS << " 个异常耗时: " 
              << duration.count() << " 微秒" << std::endl;
    
    // 验证性能在合理范围内（这个阈值可能需要根据实际情况调整）
    EXPECT_LT(duration.count(), 100000); // 少于100毫秒
}