#include "starry/Lexer.h"
#include <iostream>
#include <fstream>
#include <sstream>
#include <string>

using namespace starry;

// 将TokenType转换为字符串的辅助函数
std::string tokenTypeToString(TokenType type) {
    switch (type) {
        case TokenType::CLASS: return "CLASS";
        case TokenType::STRUCT: return "STRUCT";
        case TokenType::ENUM: return "ENUM";
        case TokenType::IF: return "IF";
        case TokenType::ELSE: return "ELSE";
        case TokenType::FOR: return "FOR";
        case TokenType::WHILE: return "WHILE";
        case TokenType::RETURN: return "RETURN";
        case TokenType::VAR: return "VAR";
        case TokenType::VAL: return "VAL";
        case TokenType::IDENTIFIER: return "IDENTIFIER";
        case TokenType::INTEGER_LITERAL: return "INTEGER_LITERAL";
        case TokenType::FLOAT_LITERAL: return "FLOAT_LITERAL";
        case TokenType::STRING_LITERAL: return "STRING_LITERAL";
        case TokenType::PLUS: return "PLUS";
        case TokenType::MINUS: return "MINUS";
        case TokenType::STAR: return "STAR";
        case TokenType::SLASH: return "SLASH";
        case TokenType::EQUAL: return "EQUAL";
        case TokenType::EQUAL_EQUAL: return "EQUAL_EQUAL";
        case TokenType::LEFT_PAREN: return "LEFT_PAREN";
        case TokenType::RIGHT_PAREN: return "RIGHT_PAREN";
        case TokenType::LEFT_BRACE: return "LEFT_BRACE";
        case TokenType::RIGHT_BRACE: return "RIGHT_BRACE";
        case TokenType::SEMICOLON: return "SEMICOLON";
        case TokenType::END_OF_FILE: return "EOF";
        default: return "OTHER";
    }
}

int main(int argc, char* argv[]) {
    std::string sourceCode;
    
    if (argc > 1) {
        // 从文件读取源代码
        std::ifstream file(argv[1]);
        if (!file) {
            std::cerr << "无法打开文件: " << argv[1] << std::endl;
            return 1;
        }
        
        std::stringstream buffer;
        buffer << file.rdbuf();
        sourceCode = buffer.str();
    } else {
        // 使用示例代码
        sourceCode = R"(
// 这是一个Starry语言示例
class Person {
    val name: str;
    var age: i32;
    
    fun constructor(name: str, age: i32) {
        this.name = name;
        this.age = age;
    }
    
    fun introduce(): str {
        return "我是 " + name + "，今年 " + age + " 岁";
    }
}

fun main() {
    var person = Person("张三", 30);
    println(person.introduce());
    
    // 测试各种字面量
    var i = 42;
    var f = 3.14;
    var s = "Hello, Starry!";
    var c = 'A';
    
    // 测试运算符
    var sum = i + 10;
    var product = i * 2;
    var isAdult = person.age >= 18;
    
    if (isAdult) {
        println("成年人");
    } else {
        println("未成年人");
    }
    
    for (var i = 0; i < 5; i++) {
        println(i);
    }
}
)";
    }
    
    // 创建词法分析器
    Lexer lexer(sourceCode);
    
    // 进行词法分析
    auto tokens = lexer.tokenize();
    
    // 打印词法分析结果
    std::cout << "词法分析结果：" << std::endl;
    std::cout << "----------------------------" << std::endl;
    std::cout << "类型\t\t词素\t\t行\t列" << std::endl;
    std::cout << "----------------------------" << std::endl;
    
    for (const auto& token : tokens) {
        std::string type = tokenTypeToString(token->getType());
        std::string lexeme = token->getLexeme();
        
        // 对于长词素，进行截断
        if (lexeme.length() > 15) {
            lexeme = lexeme.substr(0, 12) + "...";
        }
        
        std::cout << type << "\t\t" << lexeme << "\t\t" 
                  << token->getLine() << "\t" << token->getColumn() << std::endl;
    }
    
    std::cout << "----------------------------" << std::endl;
    std::cout << "共 " << tokens.size() << " 个词法单元" << std::endl;
    
    return 0;
}