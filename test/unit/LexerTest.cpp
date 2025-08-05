#include "gtest/gtest.h"
#include "starry/Lexer.h"
#include <memory>
#include <vector>

using namespace starry;

class LexerTest : public ::testing::Test {
protected:
    void SetUp() override {
        // 测试设置
    }

    void TearDown() override {
        // 测试清理
    }
    
    // 辅助函数：验证词法单元序列
    void verifyTokens(const std::string& source, const std::vector<TokenType>& expectedTypes) {
        Lexer lexer(source);
        auto tokens = lexer.tokenize();
        
        // 验证词法单元数量（包括EOF）
        ASSERT_EQ(tokens.size(), expectedTypes.size() + 1);
        
        // 验证每个词法单元的类型
        for (size_t i = 0; i < expectedTypes.size(); ++i) {
            EXPECT_EQ(tokens[i]->getType(), expectedTypes[i])
                << "位置 " << i << " 的词法单元类型不匹配";
        }
        
        // 验证最后一个词法单元是EOF
        EXPECT_EQ(tokens.back()->getType(), TokenType::END_OF_FILE);
    }
};

// 测试关键字识别
TEST_F(LexerTest, Keywords) {
    std::string source = "class struct enum if else for while return var val";
    std::vector<TokenType> expected = {
        TokenType::CLASS,
        TokenType::STRUCT,
        TokenType::ENUM,
        TokenType::IF,
        TokenType::ELSE,
        TokenType::FOR,
        TokenType::WHILE,
        TokenType::RETURN,
        TokenType::VAR,
        TokenType::VAL
    };
    
    verifyTokens(source, expected);
}

// 测试标识符识别
TEST_F(LexerTest, Identifiers) {
    std::string source = "foo bar baz _test test123";
    std::vector<TokenType> expected = {
        TokenType::IDENTIFIER,
        TokenType::IDENTIFIER,
        TokenType::IDENTIFIER,
        TokenType::IDENTIFIER,
        TokenType::IDENTIFIER
    };
    
    verifyTokens(source, expected);
}

// 测试数字字面量识别
TEST_F(LexerTest, NumberLiterals) {
    std::string source = "123 45.67 3.14159 1e10 2.5e-3";
    std::vector<TokenType> expected = {
        TokenType::INTEGER_LITERAL,
        TokenType::FLOAT_LITERAL,
        TokenType::FLOAT_LITERAL,
        TokenType::FLOAT_LITERAL,
        TokenType::FLOAT_LITERAL
    };
    
    verifyTokens(source, expected);
}

// 测试字符串字面量识别
TEST_F(LexerTest, StringLiterals) {
    std::string source = R"("Hello" "World" "Hello \"World\"")";
    std::vector<TokenType> expected = {
        TokenType::STRING_LITERAL,
        TokenType::STRING_LITERAL,
        TokenType::STRING_LITERAL
    };
    
    verifyTokens(source, expected);
}

// 测试运算符识别
TEST_F(LexerTest, Operators) {
    std::string source = "+ - * / % += -= *= /= %= ++ -- && || ! == != === !== < > <= >= & | ^ ~ &= |= ^= << >> <<= >>= ?. ?: !! .. ..< ..= :: . ? = -> => @ : ; $ _";
    std::vector<TokenType> expected = {
        TokenType::PLUS,
        TokenType::MINUS,
        TokenType::STAR,
        TokenType::SLASH,
        TokenType::PERCENT,
        TokenType::PLUS_EQUAL,
        TokenType::MINUS_EQUAL,
        TokenType::STAR_EQUAL,
        TokenType::SLASH_EQUAL,
        TokenType::PERCENT_EQUAL,
        TokenType::INCREMENT,
        TokenType::DECREMENT,
        TokenType::AND,
        TokenType::OR,
        TokenType::NOT,
        TokenType::EQUAL_EQUAL,
        TokenType::NOT_EQUAL,
        TokenType::EQUAL_EQUAL_EQUAL,
        TokenType::NOT_EQUAL_EQUAL,
        TokenType::LESS,
        TokenType::GREATER,
        TokenType::LESS_EQUAL,
        TokenType::GREATER_EQUAL,
        TokenType::BIT_AND,
        TokenType::BIT_OR,
        TokenType::BIT_XOR,
        TokenType::BIT_NOT,
        TokenType::BIT_AND_EQUAL,
        TokenType::BIT_OR_EQUAL,
        TokenType::BIT_XOR_EQUAL,
        TokenType::LEFT_SHIFT,
        TokenType::RIGHT_SHIFT,
        TokenType::LEFT_SHIFT_EQUAL,
        TokenType::RIGHT_SHIFT_EQUAL,
        TokenType::SAFE_DOT,
        TokenType::ELVIS,
        TokenType::NOT_NULL,
        TokenType::RANGE,
        TokenType::RANGE_EXCLUSIVE,
        TokenType::RANGE_INCLUSIVE,
        TokenType::SCOPE,
        TokenType::DOT,
        TokenType::QUESTION,
        TokenType::EQUAL,
        TokenType::ARROW,
        TokenType::FAT_ARROW,
        TokenType::AT,
        TokenType::COLON,
        TokenType::SEMICOLON,
        TokenType::DOLLAR,
        TokenType::UNDERSCORE
    };
    
    verifyTokens(source, expected);
}

// 测试分隔符识别
TEST_F(LexerTest, Delimiters) {
    std::string source = "( ) { } [ ] ,";
    std::vector<TokenType> expected = {
        TokenType::LEFT_PAREN,
        TokenType::RIGHT_PAREN,
        TokenType::LEFT_BRACE,
        TokenType::RIGHT_BRACE,
        TokenType::LEFT_BRACKET,
        TokenType::RIGHT_BRACKET,
        TokenType::COMMA
    };
    
    verifyTokens(source, expected);
}

// 测试注释处理
TEST_F(LexerTest, Comments) {
    std::string source = R"(
        // 这是单行注释
        var x = 10; // 行尾注释
        /* 这是
           多行注释 */
        var y = 20;
    )";
    std::vector<TokenType> expected = {
        TokenType::VAR,
        TokenType::IDENTIFIER,
        TokenType::EQUAL,
        TokenType::INTEGER_LITERAL,
        TokenType::SEMICOLON,
        TokenType::VAR,
        TokenType::IDENTIFIER,
        TokenType::EQUAL,
        TokenType::INTEGER_LITERAL,
        TokenType::SEMICOLON
    };
    
    verifyTokens(source, expected);
}

// 测试复杂代码片段
TEST_F(LexerTest, ComplexCodeSnippet) {
    std::string source = R"(
        fun calculateSum(a: int, b: int): int {
            return a + b;
        }
        
        class Person {
            val name: str;
            var age: int;
            
            constructor(name: str, age: int) {
                this.name = name;
                this.age = age;
            }
        }
    )";
    std::vector<TokenType> expected = {
        // fun calculateSum(a: int, b: int): int {
        TokenType::IDENTIFIER, // fun
        TokenType::IDENTIFIER, // calculateSum
        TokenType::LEFT_PAREN,
        TokenType::IDENTIFIER, // a
        TokenType::COLON,
        TokenType::IDENTIFIER, // int
        TokenType::COMMA,
        TokenType::IDENTIFIER, // b
        TokenType::COLON,
        TokenType::IDENTIFIER, // int
        TokenType::RIGHT_PAREN,
        TokenType::COLON,
        TokenType::IDENTIFIER, // int
        TokenType::LEFT_BRACE,
        
        // return a + b;
        TokenType::RETURN,
        TokenType::IDENTIFIER, // a
        TokenType::PLUS,
        TokenType::IDENTIFIER, // b
        TokenType::SEMICOLON,
        
        // }
        TokenType::RIGHT_BRACE,
        
        // class Person {
        TokenType::CLASS,
        TokenType::IDENTIFIER, // Person
        TokenType::LEFT_BRACE,
        
        // val name: str;
        TokenType::VAL,
        TokenType::IDENTIFIER, // name
        TokenType::COLON,
        TokenType::IDENTIFIER, // str
        TokenType::SEMICOLON,
        
        // var age: int;
        TokenType::VAR,
        TokenType::IDENTIFIER, // age
        TokenType::COLON,
        TokenType::IDENTIFIER, // int
        TokenType::SEMICOLON,
        
        // constructor(name: str, age: int) {
        TokenType::IDENTIFIER, // constructor
        TokenType::LEFT_PAREN,
        TokenType::IDENTIFIER, // name
        TokenType::COLON,
        TokenType::IDENTIFIER, // str
        TokenType::COMMA,
        TokenType::IDENTIFIER, // age
        TokenType::COLON,
        TokenType::IDENTIFIER, // int
        TokenType::RIGHT_PAREN,
        TokenType::LEFT_BRACE,
        
        // this.name = name;
        TokenType::IDENTIFIER, // this
        TokenType::DOT,
        TokenType::IDENTIFIER, // name
        TokenType::EQUAL,
        TokenType::IDENTIFIER, // name
        TokenType::SEMICOLON,
        
        // this.age = age;
        TokenType::IDENTIFIER, // this
        TokenType::DOT,
        TokenType::IDENTIFIER, // age
        TokenType::EQUAL,
        TokenType::IDENTIFIER, // age
        TokenType::SEMICOLON,
        
        // }
        TokenType::RIGHT_BRACE,
        
        // }
        TokenType::RIGHT_BRACE
    };
    
    verifyTokens(source, expected);
}

// 测试错误处理
TEST_F(LexerTest, ErrorHandling) {
    // 未闭合的字符串
    std::string source = R"("未闭合的字符串)";
    Lexer lexer(source);
    auto tokens = lexer.tokenize();
    
    // 应该只有一个错误词法单元和一个EOF
    ASSERT_EQ(tokens.size(), 2);
    EXPECT_EQ(tokens[0]->getType(), TokenType::ERROR);
    EXPECT_EQ(tokens[1]->getType(), TokenType::END_OF_FILE);
}