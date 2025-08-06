/**
 * @file StringTest.cpp
 * @brief Starry语言字符串库单元测试
 * @author Starry Team
 * @date 2024
 */

#include <gtest/gtest.h>
#include "starry/stdlib/String.h"
#include <vector>

using namespace starry::stdlib;

class StringTest : public ::testing::Test {
protected:
    void SetUp() override {
        // 测试设置
    }
    
    void TearDown() override {
        // 测试清理
    }
};

// 测试字符串构造
TEST_F(StringTest, ConstructorTest) {
    // 默认构造
    StarryString empty;
    EXPECT_TRUE(empty.empty());
    EXPECT_EQ(empty.length(), 0);
    
    // 从std::string构造
    StarryString from_std("Hello");
    EXPECT_EQ(from_std.str(), "Hello");
    EXPECT_EQ(from_std.length(), 5);
    
    // 从C字符串构造
    StarryString from_cstr("World");
    EXPECT_EQ(from_cstr.str(), "World");
    EXPECT_EQ(from_cstr.length(), 5);
    
    // 从空指针构造
    StarryString from_null(nullptr);
    EXPECT_TRUE(from_null.empty());
}

// 测试拷贝构造和赋值
TEST_F(StringTest, CopyTest) {
    StarryString original("Original");
    
    // 拷贝构造
    StarryString copy(original);
    EXPECT_EQ(copy.str(), "Original");
    EXPECT_EQ(copy.length(), 8);
    
    // 拷贝赋值
    StarryString assigned;
    assigned = original;
    EXPECT_EQ(assigned.str(), "Original");
    EXPECT_EQ(assigned.length(), 8);
    
    // 修改原字符串不应影响拷贝
    StarryString modified = original;
    modified = StarryString("Modified");
    EXPECT_EQ(original.str(), "Original");
    EXPECT_EQ(modified.str(), "Modified");
}

// 测试移动构造和赋值
TEST_F(StringTest, MoveTest) {
    StarryString original("MoveTest");
    std::string original_content = original.str();
    
    // 移动构造
    StarryString moved(std::move(original));
    EXPECT_EQ(moved.str(), original_content);
    
    // 移动赋值
    StarryString move_assigned;
    StarryString source("MoveAssign");
    std::string source_content = source.str();
    move_assigned = std::move(source);
    EXPECT_EQ(move_assigned.str(), source_content);
}

// 测试基本属性
TEST_F(StringTest, BasicPropertiesTest) {
    StarryString str("Hello World");
    
    EXPECT_EQ(str.length(), 11);
    EXPECT_FALSE(str.empty());
    EXPECT_STREQ(str.c_str(), "Hello World");
    EXPECT_EQ(str.str(), "Hello World");
    
    StarryString empty_str;
    EXPECT_EQ(empty_str.length(), 0);
    EXPECT_TRUE(empty_str.empty());
}

// 测试子字符串
TEST_F(StringTest, SubstringTest) {
    StarryString str("Hello World");
    
    // 正常子字符串
    StarryString sub1 = str.substring(0, 5);
    EXPECT_EQ(sub1.str(), "Hello");
    
    StarryString sub2 = str.substring(6, 5);
    EXPECT_EQ(sub2.str(), "World");
    
    // 从中间开始到结尾
    StarryString sub3 = str.substring(6);
    EXPECT_EQ(sub3.str(), "World");
    
    // 超出范围的起始位置
    StarryString sub4 = str.substring(20);
    EXPECT_TRUE(sub4.empty());
    
    // 长度超出字符串末尾
    StarryString sub5 = str.substring(6, 100);
    EXPECT_EQ(sub5.str(), "World");
}

// 测试查找功能
TEST_F(StringTest, SearchTest) {
    StarryString str("Hello World Hello");
    
    // 查找存在的子字符串
    EXPECT_EQ(str.indexOf(StarryString("Hello")), 0);
    EXPECT_EQ(str.indexOf(StarryString("World")), 6);
    EXPECT_EQ(str.indexOf(StarryString("o")), 4);
    
    // 查找不存在的子字符串
    EXPECT_EQ(str.indexOf(StarryString("xyz")), SIZE_MAX);
    
    // 测试包含功能
    EXPECT_TRUE(str.contains(StarryString("Hello")));
    EXPECT_TRUE(str.contains(StarryString("World")));
    EXPECT_TRUE(str.contains(StarryString("o")));
    EXPECT_FALSE(str.contains(StarryString("xyz")));
}

// 测试大小写转换
TEST_F(StringTest, CaseConversionTest) {
    StarryString str("Hello World 123");
    
    StarryString lower = str.toLowerCase();
    EXPECT_EQ(lower.str(), "hello world 123");
    
    StarryString upper = str.toUpperCase();
    EXPECT_EQ(upper.str(), "HELLO WORLD 123");
    
    // 原字符串不应改变
    EXPECT_EQ(str.str(), "Hello World 123");
}

// 测试去除空白
TEST_F(StringTest, TrimTest) {
    StarryString str1("  Hello World  ");
    StarryString trimmed1 = str1.trim();
    EXPECT_EQ(trimmed1.str(), "Hello World");
    
    StarryString str2("\t\n  Hello  \r\n  ");
    StarryString trimmed2 = str2.trim();
    EXPECT_EQ(trimmed2.str(), "Hello");
    
    StarryString str3("NoSpaces");
    StarryString trimmed3 = str3.trim();
    EXPECT_EQ(trimmed3.str(), "NoSpaces");
    
    StarryString str4("   ");
    StarryString trimmed4 = str4.trim();
    EXPECT_TRUE(trimmed4.empty());
}

// 测试字符串分割
TEST_F(StringTest, SplitTest) {
    StarryString str("apple,banana,cherry");
    std::vector<StarryString> parts = str.split(StarryString(","));
    
    EXPECT_EQ(parts.size(), 3);
    EXPECT_EQ(parts[0].str(), "apple");
    EXPECT_EQ(parts[1].str(), "banana");
    EXPECT_EQ(parts[2].str(), "cherry");
    
    // 测试多字符分隔符
    StarryString str2("one::two::three");
    std::vector<StarryString> parts2 = str2.split(StarryString("::"));
    
    EXPECT_EQ(parts2.size(), 3);
    EXPECT_EQ(parts2[0].str(), "one");
    EXPECT_EQ(parts2[1].str(), "two");
    EXPECT_EQ(parts2[2].str(), "three");
    
    // 测试不存在的分隔符
    StarryString str3("noseparator");
    std::vector<StarryString> parts3 = str3.split(StarryString(","));
    
    EXPECT_EQ(parts3.size(), 1);
    EXPECT_EQ(parts3[0].str(), "noseparator");
}

// 测试字符串替换
TEST_F(StringTest, ReplaceTest) {
    StarryString str("Hello World Hello");
    
    // 替换所有匹配
    StarryString replaced1 = str.replace(StarryString("Hello"), StarryString("Hi"));
    EXPECT_EQ(replaced1.str(), "Hi World Hi");
    
    // 替换单个字符
    StarryString replaced2 = str.replace(StarryString("o"), StarryString("0"));
    EXPECT_EQ(replaced2.str(), "Hell0 W0rld Hell0");
    
    // 替换不存在的子字符串
    StarryString replaced3 = str.replace(StarryString("xyz"), StarryString("abc"));
    EXPECT_EQ(replaced3.str(), "Hello World Hello");
    
    // 原字符串不应改变
    EXPECT_EQ(str.str(), "Hello World Hello");
}

// 测试运算符重载
TEST_F(StringTest, OperatorTest) {
    StarryString str1("Hello");
    StarryString str2(" World");
    StarryString str3("Hello");
    
    // 加法运算符
    StarryString result = str1 + str2;
    EXPECT_EQ(result.str(), "Hello World");
    
    // 加等运算符
    StarryString str4("Hello");
    str4 += str2;
    EXPECT_EQ(str4.str(), "Hello World");
    
    // 比较运算符
    EXPECT_TRUE(str1 == str3);
    EXPECT_FALSE(str1 == str2);
    EXPECT_TRUE(str1 != str2);
    EXPECT_FALSE(str1 != str3);
    EXPECT_TRUE(str1 < str2); // "Hello" < " World" (按ASCII)
    
    // 索引运算符
    EXPECT_EQ(str1[0], 'H');
    EXPECT_EQ(str1[1], 'e');
    EXPECT_EQ(str1[4], 'o');
    
    // 超出范围的索引应该抛出异常
    EXPECT_THROW(str1[10], std::out_of_range);
}

// 测试类型转换函数
TEST_F(StringTest, ConversionTest) {
    // 整数转换
    EXPECT_EQ(toString(42).str(), "42");
    EXPECT_EQ(toString(-123).str(), "-123");
    
    // 浮点数转换
    StarryString double_str = toString(3.14);
    EXPECT_TRUE(double_str.contains(StarryString("3.14")));
    
    // 布尔转换
    EXPECT_EQ(toString(true).str(), "true");
    EXPECT_EQ(toString(false).str(), "false");
    
    // 字符串转整数
    EXPECT_EQ(toInt(StarryString("42")), 42);
    EXPECT_EQ(toInt(StarryString("-123")), -123);
    EXPECT_EQ(toInt(StarryString("invalid")), 0);
    
    // 字符串转浮点数
    EXPECT_DOUBLE_EQ(toDouble(StarryString("3.14")), 3.14);
    EXPECT_DOUBLE_EQ(toDouble(StarryString("-2.5")), -2.5);
    EXPECT_DOUBLE_EQ(toDouble(StarryString("invalid")), 0.0);
    
    // 字符串转布尔
    EXPECT_TRUE(toBool(StarryString("true")));
    EXPECT_TRUE(toBool(StarryString("TRUE")));
    EXPECT_TRUE(toBool(StarryString("1")));
    EXPECT_TRUE(toBool(StarryString("yes")));
    EXPECT_FALSE(toBool(StarryString("false")));
    EXPECT_FALSE(toBool(StarryString("0")));
    EXPECT_FALSE(toBool(StarryString("no")));
}

// 测试边界情况
TEST_F(StringTest, EdgeCasesTest) {
    // 空字符串操作
    StarryString empty;
    EXPECT_EQ(empty.substring(0, 5).str(), "");
    EXPECT_EQ(empty.indexOf(StarryString("x")), SIZE_MAX);
    EXPECT_FALSE(empty.contains(StarryString("x")));
    EXPECT_EQ(empty.toLowerCase().str(), "");
    EXPECT_EQ(empty.toUpperCase().str(), "");
    EXPECT_EQ(empty.trim().str(), "");
    
    // 单字符字符串
    StarryString single("A");
    EXPECT_EQ(single.length(), 1);
    EXPECT_EQ(single[0], 'A');
    EXPECT_EQ(single.toLowerCase().str(), "a");
    EXPECT_EQ(single.toUpperCase().str(), "A");
}