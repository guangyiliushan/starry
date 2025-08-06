#include <gtest/gtest.h>
#include "starry/stdlib/Math.h"
#include <cmath>
#include <vector>
#include <chrono>

using namespace starry::stdlib;

class MathTest : public ::testing::Test {
protected:
    void SetUp() override {
        // 设置随机种子以确保测试的可重复性
        Math::setSeed(12345);
    }
    
    void TearDown() override {
        // 清理
    }
    
    // 浮点数比较辅助函数
    bool isClose(double a, double b, double tolerance = 1e-9) {
        return std::abs(a - b) < tolerance;
    }
};

// 测试数学常量
TEST_F(MathTest, MathConstants) {
    EXPECT_TRUE(isClose(Math::PI, 3.14159265358979323846));
    EXPECT_TRUE(isClose(Math::E, 2.71828182845904523536));
    EXPECT_TRUE(isClose(Math::SQRT2, 1.41421356237309504880));
    EXPECT_TRUE(isClose(Math::SQRT3, 1.73205080756887729353));
    EXPECT_TRUE(isClose(Math::LN2, 0.69314718055994530942));
    EXPECT_TRUE(isClose(Math::LN10, 2.30258509299404568402));
}

// 测试绝对值函数
TEST_F(MathTest, AbsoluteValue) {
    EXPECT_EQ(Math::abs(5), 5);
    EXPECT_EQ(Math::abs(-5), 5);
    EXPECT_EQ(Math::abs(0), 0);
    
    EXPECT_TRUE(isClose(Math::abs(3.14), 3.14));
    EXPECT_TRUE(isClose(Math::abs(-3.14), 3.14));
    EXPECT_TRUE(isClose(Math::abs(0.0), 0.0));
}

// 测试平方根函数
TEST_F(MathTest, SquareRoot) {
    EXPECT_TRUE(isClose(Math::sqrt(4.0), 2.0));
    EXPECT_TRUE(isClose(Math::sqrt(9.0), 3.0));
    EXPECT_TRUE(isClose(Math::sqrt(2.0), Math::SQRT2));
    EXPECT_TRUE(isClose(Math::sqrt(0.0), 0.0));
    
    // 测试负数输入应该抛出异常
    EXPECT_THROW(Math::sqrt(-1.0), std::invalid_argument);
}

// 测试幂函数
TEST_F(MathTest, PowerFunction) {
    EXPECT_TRUE(isClose(Math::pow(2.0, 3.0), 8.0));
    EXPECT_TRUE(isClose(Math::pow(5.0, 0.0), 1.0));
    EXPECT_TRUE(isClose(Math::pow(2.0, -1.0), 0.5));
    EXPECT_TRUE(isClose(Math::pow(4.0, 0.5), 2.0));
}

// 测试指数函数
TEST_F(MathTest, ExponentialFunction) {
    EXPECT_TRUE(isClose(Math::exp(0.0), 1.0));
    EXPECT_TRUE(isClose(Math::exp(1.0), Math::E));
    EXPECT_TRUE(isClose(Math::exp(2.0), Math::E * Math::E));
}

// 测试对数函数
TEST_F(MathTest, LogarithmFunctions) {
    EXPECT_TRUE(isClose(Math::log(1.0), 0.0));
    EXPECT_TRUE(isClose(Math::log(Math::E), 1.0));
    EXPECT_TRUE(isClose(Math::log10(1.0), 0.0));
    EXPECT_TRUE(isClose(Math::log10(10.0), 1.0));
    EXPECT_TRUE(isClose(Math::log2(1.0), 0.0));
    EXPECT_TRUE(isClose(Math::log2(2.0), 1.0));
    
    // 测试非正数输入应该抛出异常
    EXPECT_THROW(Math::log(0.0), std::invalid_argument);
    EXPECT_THROW(Math::log(-1.0), std::invalid_argument);
    EXPECT_THROW(Math::log10(0.0), std::invalid_argument);
    EXPECT_THROW(Math::log10(-1.0), std::invalid_argument);
    EXPECT_THROW(Math::log2(0.0), std::invalid_argument);
    EXPECT_THROW(Math::log2(-1.0), std::invalid_argument);
}

// 测试三角函数
TEST_F(MathTest, TrigonometricFunctions) {
    EXPECT_TRUE(isClose(Math::sin(0.0), 0.0));
    EXPECT_TRUE(isClose(Math::sin(Math::PI / 2), 1.0));
    EXPECT_TRUE(isClose(Math::cos(0.0), 1.0));
    EXPECT_TRUE(isClose(Math::cos(Math::PI), -1.0));
    EXPECT_TRUE(isClose(Math::tan(0.0), 0.0));
    EXPECT_TRUE(isClose(Math::tan(Math::PI / 4), 1.0, 1e-8));
}

// 测试反三角函数
TEST_F(MathTest, InverseTrigonometricFunctions) {
    EXPECT_TRUE(isClose(Math::asin(0.0), 0.0));
    EXPECT_TRUE(isClose(Math::asin(1.0), Math::PI / 2));
    EXPECT_TRUE(isClose(Math::acos(1.0), 0.0));
    EXPECT_TRUE(isClose(Math::acos(0.0), Math::PI / 2));
    EXPECT_TRUE(isClose(Math::atan(0.0), 0.0));
    EXPECT_TRUE(isClose(Math::atan(1.0), Math::PI / 4));
    
    // 测试超出范围的输入
    EXPECT_THROW(Math::asin(2.0), std::invalid_argument);
    EXPECT_THROW(Math::asin(-2.0), std::invalid_argument);
    EXPECT_THROW(Math::acos(2.0), std::invalid_argument);
    EXPECT_THROW(Math::acos(-2.0), std::invalid_argument);
}

// 测试atan2函数
TEST_F(MathTest, Atan2Function) {
    EXPECT_TRUE(isClose(Math::atan2(0.0, 1.0), 0.0));
    EXPECT_TRUE(isClose(Math::atan2(1.0, 0.0), Math::PI / 2));
    EXPECT_TRUE(isClose(Math::atan2(0.0, -1.0), Math::PI));
    EXPECT_TRUE(isClose(Math::atan2(-1.0, 0.0), -Math::PI / 2));
}

// 测试双曲函数
TEST_F(MathTest, HyperbolicFunctions) {
    EXPECT_TRUE(isClose(Math::sinh(0.0), 0.0));
    EXPECT_TRUE(isClose(Math::cosh(0.0), 1.0));
    EXPECT_TRUE(isClose(Math::tanh(0.0), 0.0));
    
    EXPECT_TRUE(isClose(Math::asinh(0.0), 0.0));
    EXPECT_TRUE(isClose(Math::acosh(1.0), 0.0));
    EXPECT_TRUE(isClose(Math::atanh(0.0), 0.0));
    
    // 测试超出范围的输入
    EXPECT_THROW(Math::acosh(0.5), std::invalid_argument);
    EXPECT_THROW(Math::atanh(1.0), std::invalid_argument);
    EXPECT_THROW(Math::atanh(-1.0), std::invalid_argument);
}

// 测试取整函数
TEST_F(MathTest, RoundingFunctions) {
    EXPECT_TRUE(isClose(Math::floor(3.7), 3.0));
    EXPECT_TRUE(isClose(Math::floor(-3.7), -4.0));
    EXPECT_TRUE(isClose(Math::ceil(3.2), 4.0));
    EXPECT_TRUE(isClose(Math::ceil(-3.2), -3.0));
    EXPECT_TRUE(isClose(Math::round(3.5), 4.0));
    EXPECT_TRUE(isClose(Math::round(3.4), 3.0));
    EXPECT_TRUE(isClose(Math::trunc(3.7), 3.0));
    EXPECT_TRUE(isClose(Math::trunc(-3.7), -3.0));
}

// 测试比较函数
TEST_F(MathTest, ComparisonFunctions) {
    EXPECT_EQ(Math::max(5, 3), 5);
    EXPECT_EQ(Math::max(-2, -5), -2);
    EXPECT_EQ(Math::min(5, 3), 3);
    EXPECT_EQ(Math::min(-2, -5), -5);
    
    EXPECT_TRUE(isClose(Math::max(3.14, 2.71), 3.14));
    EXPECT_TRUE(isClose(Math::min(3.14, 2.71), 2.71));
}

// 测试符号函数
TEST_F(MathTest, SignFunction) {
    EXPECT_EQ(Math::sign(5.0), 1);
    EXPECT_EQ(Math::sign(-3.0), -1);
    EXPECT_EQ(Math::sign(0.0), 0);
}

// 测试判断函数
TEST_F(MathTest, TestingFunctions) {
    EXPECT_FALSE(Math::isNaN(5.0));
    EXPECT_TRUE(Math::isNaN(std::numeric_limits<double>::quiet_NaN()));
    
    EXPECT_FALSE(Math::isInfinite(5.0));
    EXPECT_TRUE(Math::isInfinite(std::numeric_limits<double>::infinity()));
    
    EXPECT_TRUE(Math::isFinite(5.0));
    EXPECT_FALSE(Math::isFinite(std::numeric_limits<double>::infinity()));
}

// 测试随机数函数
TEST_F(MathTest, RandomFunctions) {
    // 测试random()返回值在[0,1)范围内
    for (int i = 0; i < 100; ++i) {
        double r = Math::random();
        EXPECT_GE(r, 0.0);
        EXPECT_LT(r, 1.0);
    }
    
    // 测试randomInt
    for (int i = 0; i < 100; ++i) {
        int r = Math::randomInt(1, 10);
        EXPECT_GE(r, 1);
        EXPECT_LE(r, 10);
    }
    
    // 测试randomDouble
    for (int i = 0; i < 100; ++i) {
        double r = Math::randomDouble(1.0, 10.0);
        EXPECT_GE(r, 1.0);
        EXPECT_LT(r, 10.0);
    }
    
    // 测试无效范围
    EXPECT_THROW(Math::randomInt(10, 5), std::invalid_argument);
    EXPECT_THROW(Math::randomDouble(10.0, 5.0), std::invalid_argument);
}

// 测试角度转换
TEST_F(MathTest, AngleConversion) {
    EXPECT_TRUE(isClose(Math::toRadians(180.0), Math::PI));
    EXPECT_TRUE(isClose(Math::toRadians(90.0), Math::PI / 2));
    EXPECT_TRUE(isClose(Math::toDegrees(Math::PI), 180.0));
    EXPECT_TRUE(isClose(Math::toDegrees(Math::PI / 2), 90.0));
}

// 测试数值处理函数
TEST_F(MathTest, ValueProcessing) {
    EXPECT_TRUE(isClose(Math::clamp(5.0, 1.0, 10.0), 5.0));
    EXPECT_TRUE(isClose(Math::clamp(-5.0, 1.0, 10.0), 1.0));
    EXPECT_TRUE(isClose(Math::clamp(15.0, 1.0, 10.0), 10.0));
    
    EXPECT_THROW(Math::clamp(5.0, 10.0, 1.0), std::invalid_argument);
    
    EXPECT_TRUE(isClose(Math::lerp(0.0, 10.0, 0.5), 5.0));
    EXPECT_TRUE(isClose(Math::lerp(0.0, 10.0, 0.0), 0.0));
    EXPECT_TRUE(isClose(Math::lerp(0.0, 10.0, 1.0), 10.0));
    
    EXPECT_TRUE(isClose(Math::map(5.0, 0.0, 10.0, 0.0, 100.0), 50.0));
    EXPECT_THROW(Math::map(5.0, 5.0, 5.0, 0.0, 100.0), std::invalid_argument);
}

// 测试统计函数
TEST_F(MathTest, StatisticalFunctions) {
    std::vector<double> values = {1.0, 2.0, 3.0, 4.0, 5.0};
    
    EXPECT_TRUE(isClose(Math::sum(values), 15.0));
    EXPECT_TRUE(isClose(Math::mean(values), 3.0));
    EXPECT_TRUE(isClose(Math::median(values), 3.0));
    
    std::vector<double> evenValues = {1.0, 2.0, 3.0, 4.0};
    EXPECT_TRUE(isClose(Math::median(evenValues), 2.5));
    
    EXPECT_TRUE(isClose(Math::variance(values), 2.5));
    EXPECT_TRUE(isClose(Math::standardDeviation(values), std::sqrt(2.5)));
    
    // 测试空向量
    std::vector<double> empty;
    EXPECT_THROW(Math::mean(empty), std::invalid_argument);
    EXPECT_THROW(Math::median(empty), std::invalid_argument);
    
    // 测试单个元素
    std::vector<double> single = {5.0};
    EXPECT_THROW(Math::variance(single), std::invalid_argument);
}

// 测试数论函数
TEST_F(MathTest, NumberTheoryFunctions) {
    EXPECT_EQ(Math::gcd(12, 8), 4);
    EXPECT_EQ(Math::gcd(17, 13), 1);
    EXPECT_EQ(Math::gcd(-12, 8), 4);
    EXPECT_EQ(Math::gcd(0, 5), 5);
    
    EXPECT_EQ(Math::lcm(12, 8), 24);
    EXPECT_EQ(Math::lcm(17, 13), 221);
    EXPECT_EQ(Math::lcm(0, 5), 0);
    
    EXPECT_TRUE(Math::isPrime(2));
    EXPECT_TRUE(Math::isPrime(17));
    EXPECT_FALSE(Math::isPrime(1));
    EXPECT_FALSE(Math::isPrime(4));
    EXPECT_FALSE(Math::isPrime(-5));
    
    EXPECT_EQ(Math::factorial(0), 1);
    EXPECT_EQ(Math::factorial(5), 120);
    EXPECT_THROW(Math::factorial(-1), std::invalid_argument);
    EXPECT_THROW(Math::factorial(25), std::invalid_argument);
    
    EXPECT_EQ(Math::fibonacci(0), 0);
    EXPECT_EQ(Math::fibonacci(1), 1);
    EXPECT_EQ(Math::fibonacci(10), 55);
    EXPECT_THROW(Math::fibonacci(-1), std::invalid_argument);
}

// 测试组合数学
TEST_F(MathTest, CombinatoricsFunctions) {
    EXPECT_EQ(Math::combination(5, 2), 10);
    EXPECT_EQ(Math::combination(5, 0), 1);
    EXPECT_EQ(Math::combination(5, 5), 1);
    EXPECT_EQ(Math::combination(5, 6), 0);
    
    EXPECT_EQ(Math::permutation(5, 2), 20);
    EXPECT_EQ(Math::permutation(5, 0), 1);
    EXPECT_EQ(Math::permutation(5, 5), 120);
    EXPECT_EQ(Math::permutation(5, 6), 0);
}

// 性能测试
TEST_F(MathTest, PerformanceTest) {
    const int NUM_OPERATIONS = 100000;
    
    auto start = std::chrono::high_resolution_clock::now();
    
    for (int i = 0; i < NUM_OPERATIONS; ++i) {
        double x = i * 0.001;
        volatile double result = Math::sin(x) + Math::cos(x) + Math::sqrt(x + 1);
        (void)result; // 避免编译器优化
    }
    
    auto end = std::chrono::high_resolution_clock::now();
    auto duration = std::chrono::duration_cast<std::chrono::microseconds>(end - start);
    
    std::cout << "执行 " << NUM_OPERATIONS << " 次数学运算耗时: " 
              << duration.count() << " 微秒" << std::endl;
    
    // 验证性能在合理范围内
    EXPECT_LT(duration.count(), 1000000); // 少于1秒
}

// 测试随机数种子设置
TEST_F(MathTest, RandomSeedTest) {
    // 设置相同种子应该产生相同的随机数序列
    Math::setSeed(42);
    std::vector<double> sequence1;
    for (int i = 0; i < 10; ++i) {
        sequence1.push_back(Math::random());
    }
    
    Math::setSeed(42);
    std::vector<double> sequence2;
    for (int i = 0; i < 10; ++i) {
        sequence2.push_back(Math::random());
    }
    
    EXPECT_EQ(sequence1.size(), sequence2.size());
    for (size_t i = 0; i < sequence1.size(); ++i) {
        EXPECT_TRUE(isClose(sequence1[i], sequence2[i]));
    }
}

// 边界值测试
TEST_F(MathTest, BoundaryValueTest) {
    // 测试极大值和极小值
    EXPECT_TRUE(Math::isFinite(std::numeric_limits<double>::max()));
    EXPECT_TRUE(Math::isFinite(std::numeric_limits<double>::lowest()));
    
    // 测试接近零的值
    double tiny = std::numeric_limits<double>::epsilon();
    EXPECT_TRUE(Math::isFinite(tiny));
    EXPECT_GT(Math::abs(tiny), 0.0);
    
    // 测试特殊值
    EXPECT_TRUE(Math::isNaN(0.0 / 0.0));
    EXPECT_TRUE(Math::isInfinite(1.0 / 0.0));
}