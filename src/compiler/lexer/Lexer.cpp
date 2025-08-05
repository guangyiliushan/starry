#include "starry/Lexer.h"
#include <sstream>
#include <iostream>

namespace starry {

// 初始化关键字映射表
std::unordered_map<std::string, TokenType> Lexer::keywords = {
    {"class", TokenType::CLASS},
    {"struct", TokenType::STRUCT},
    {"enum", TokenType::ENUM},
    {"union", TokenType::UNION},
    {"typedef", TokenType::TYPEDEF},
    {"using", TokenType::USING},
    {"if", TokenType::IF},
    {"else", TokenType::ELSE},
    {"switch", TokenType::SWITCH},
    {"case", TokenType::CASE},
    {"default", TokenType::DEFAULT},
    {"for", TokenType::FOR},
    {"while", TokenType::WHILE},
    {"do", TokenType::DO},
    {"break", TokenType::BREAK},
    {"continue", TokenType::CONTINUE},
    {"return", TokenType::RETURN},
    {"goto", TokenType::GOTO},
    {"try", TokenType::TRY},
    {"catch", TokenType::CATCH},
    {"throw", TokenType::THROW},
    {"const", TokenType::CONST},
    {"volatile", TokenType::VOLATILE},
    {"static", TokenType::STATIC},
    {"extern", TokenType::EXTERN},
    {"inline", TokenType::INLINE},
    {"virtual", TokenType::VIRTUAL},
    {"explicit", TokenType::EXPLICIT},
    {"friend", TokenType::FRIEND},
    {"mutable", TokenType::MUTABLE},
    {"public", TokenType::PUBLIC},
    {"private", TokenType::PRIVATE},
    {"protected", TokenType::PROTECTED},
    {"new", TokenType::NEW},
    {"delete", TokenType::DELETE},
    {"sizeof", TokenType::SIZEOF},
    {"template", TokenType::TEMPLATE},
    {"typename", TokenType::TYPENAME},
    {"namespace", TokenType::NAMESPACE},
    {"true", TokenType::TRUE},
    {"false", TokenType::FALSE},
    {"null", TokenType::NULL_LITERAL},
    {"var", TokenType::VAR},
    {"val", TokenType::VAL},
    {"is", TokenType::IS},
    {"as", TokenType::AS},
    {"extension", TokenType::EXTENSION}
};

// Token类的toString方法实现
std::string Token::toString() const {
    std::stringstream ss;
    ss << "Token(type=" << static_cast<int>(type) 
       << ", lexeme='" << lexeme 
       << "', line=" << line 
       << ", column=" << column << ")";
    return ss.str();
}

// Lexer构造函数
Lexer::Lexer(const std::string& source) : source(source) {}

// 词法分析主方法
std::vector<std::shared_ptr<Token>> Lexer::tokenize() {
    std::vector<std::shared_ptr<Token>> tokens;
    
    while (!isAtEnd()) {
        // 记录当前词法单元的起始位置
        start = current;
        
        // 扫描下一个词法单元
        std::shared_ptr<Token> token = scanToken();
        
        // 如果是有效的词法单元，添加到结果列表中
        if (token->getType() != TokenType::ERROR) {
            tokens.push_back(token);
        } else {
            // 输出错误信息
            std::cerr << "词法错误: " << token->getLexeme() 
                      << " 在第 " << token->getLine() 
                      << " 行, 第 " << token->getColumn() << " 列" << std::endl;
        }
    }
    
    // 添加文件结束标记
    tokens.push_back(std::make_shared<Token>(TokenType::END_OF_FILE, "", line, column));
    
    return tokens;
}

// 判断是否到达源代码末尾
bool Lexer::isAtEnd() const {
    return current >= static_cast<int>(source.length());
}

// 获取当前字符并前进
char Lexer::advance() {
    char c = source[current++];
    if (c == '\n') {
        line++;
        column = 1;
    } else {
        column++;
    }
    return c;
}

// 查看当前字符但不前进
char Lexer::peek() const {
    if (isAtEnd()) return '\0';
    return source[current];
}

// 查看下一个字符但不前进
char Lexer::peekNext() const {
    if (current + 1 >= static_cast<int>(source.length())) return '\0';
    return source[current + 1];
}

// 如果当前字符匹配预期，则前进并返回true
bool Lexer::match(char expected) {
    if (isAtEnd()) return false;
    if (source[current] != expected) return false;
    
    current++;
    column++;
    return true;
}

// 扫描并返回下一个词法单元
std::shared_ptr<Token> Lexer::scanToken() {
    // 跳过空白字符和注释
    skipWhitespace();
    
    // 如果到达源代码末尾，返回EOF标记
    if (isAtEnd()) {
        return makeToken(TokenType::END_OF_FILE);
    }
    
    char c = advance();
    
    // 标识符或关键字
    if (isAlpha(c) || c == '_') {
        return identifier();
    }
    
    // 数字
    if (isDigit(c)) {
        return number();
    }
    
    // 根据字符确定词法单元类型
    switch (c) {
        // 单字符词法单元
        case '(': return makeToken(TokenType::LEFT_PAREN);
        case ')': return makeToken(TokenType::RIGHT_PAREN);
        case '{': return makeToken(TokenType::LEFT_BRACE);
        case '}': return makeToken(TokenType::RIGHT_BRACE);
        case '[': return makeToken(TokenType::LEFT_BRACKET);
        case ']': return makeToken(TokenType::RIGHT_BRACKET);
        case ',': return makeToken(TokenType::COMMA);
        case '.': 
            if (match('.')) {
                if (match('<')) return makeToken(TokenType::RANGE_EXCLUSIVE);
                if (match('=')) return makeToken(TokenType::RANGE_INCLUSIVE);
                return makeToken(TokenType::RANGE);
            }
            return makeToken(TokenType::DOT);
        case '-': 
            if (match('=')) return makeToken(TokenType::MINUS_EQUAL);
            if (match('>')) return makeToken(TokenType::ARROW);
            if (match('-')) return makeToken(TokenType::DECREMENT);
            return makeToken(TokenType::MINUS);
        case '+': 
            if (match('=')) return makeToken(TokenType::PLUS_EQUAL);
            if (match('+')) return makeToken(TokenType::INCREMENT);
            return makeToken(TokenType::PLUS);
        case '*': 
            if (match('=')) return makeToken(TokenType::STAR_EQUAL);
            return makeToken(TokenType::STAR);
        case '/': 
            if (match('/')) {
                // 单行注释
                while (peek() != '\n' && !isAtEnd()) advance();
                return scanToken();
            } else if (match('*')) {
                // 多行注释
                skipComment();
                return scanToken();
            } else if (match('=')) {
                return makeToken(TokenType::SLASH_EQUAL);
            }
            return makeToken(TokenType::SLASH);
        case '%': 
            if (match('=')) return makeToken(TokenType::PERCENT_EQUAL);
            return makeToken(TokenType::PERCENT);
        case '!': 
            if (match('=')) {
                if (match('=')) return makeToken(TokenType::NOT_EQUAL_EQUAL);
                return makeToken(TokenType::NOT_EQUAL);
            }
            if (match('!')) return makeToken(TokenType::NOT_NULL);
            return makeToken(TokenType::NOT);
        case '=': 
            if (match('=')) {
                if (match('=')) return makeToken(TokenType::EQUAL_EQUAL_EQUAL);
                return makeToken(TokenType::EQUAL_EQUAL);
            }
            if (match('>')) return makeToken(TokenType::FAT_ARROW);
            return makeToken(TokenType::EQUAL);
        case '<': 
            if (match('=')) return makeToken(TokenType::LESS_EQUAL);
            if (match('<')) {
                if (match('=')) return makeToken(TokenType::LEFT_SHIFT_EQUAL);
                return makeToken(TokenType::LEFT_SHIFT);
            }
            return makeToken(TokenType::LESS);
        case '>': 
            if (match('=')) return makeToken(TokenType::GREATER_EQUAL);
            if (match('>')) {
                if (match('=')) return makeToken(TokenType::RIGHT_SHIFT_EQUAL);
                return makeToken(TokenType::RIGHT_SHIFT);
            }
            return makeToken(TokenType::GREATER);
        case '&': 
            if (match('&')) return makeToken(TokenType::AND);
            if (match('=')) return makeToken(TokenType::BIT_AND_EQUAL);
            return makeToken(TokenType::BIT_AND);
        case '|': 
            if (match('|')) return makeToken(TokenType::OR);
            if (match('=')) return makeToken(TokenType::BIT_OR_EQUAL);
            return makeToken(TokenType::BIT_OR);
        case '^': 
            if (match('=')) return makeToken(TokenType::BIT_XOR_EQUAL);
            return makeToken(TokenType::BIT_XOR);
        case '~': return makeToken(TokenType::BIT_NOT);
        case '?': 
            if (match('.')) return makeToken(TokenType::SAFE_DOT);
            if (match(':')) return makeToken(TokenType::ELVIS);
            return makeToken(TokenType::QUESTION);
        case ':': 
            if (match(':')) return makeToken(TokenType::SCOPE);
            return makeToken(TokenType::COLON);
        case ';': return makeToken(TokenType::SEMICOLON);
        case '@': return makeToken(TokenType::AT);
        case '$': return makeToken(TokenType::DOLLAR);
        case '_': return makeToken(TokenType::UNDERSCORE);
        
        // 字符串字面量
        case '"': return string();
        
        // 字符字面量
        case '\'': return character();
        
        default:
            return errorToken("意外的字符");
    }
}

// 处理标识符和关键字
std::shared_ptr<Token> Lexer::identifier() {
    while (isAlphaNumeric(peek())) advance();
    
    // 提取标识符文本
    std::string text = source.substr(start, current - start);
    
    // 检查是否是关键字
    auto it = keywords.find(text);
    TokenType type = (it != keywords.end()) ? it->second : TokenType::IDENTIFIER;
    
    return makeToken(type);
}

// 处理数字字面量
std::shared_ptr<Token> Lexer::number() {
    bool isFloat = false;
    
    // 处理整数部分
    while (isDigit(peek())) advance();
    
    // 处理小数部分
    if (peek() == '.' && isDigit(peekNext())) {
        isFloat = true;
        
        // 消费小数点
        advance();
        
        // 消费小数部分
        while (isDigit(peek())) advance();
    }
    
    // 处理科学计数法
    if (peek() == 'e' || peek() == 'E') {
        isFloat = true;
        
        // 消费'e'或'E'
        advance();
        
        // 处理可选的符号
        if (peek() == '+' || peek() == '-') advance();
        
        // 必须至少有一个数字
        if (!isDigit(peek())) {
            return errorToken("无效的科学计数法表示");
        }
        
        // 消费指数部分
        while (isDigit(peek())) advance();
    }
    
    // 处理后缀
    if (peek() == 'f' || peek() == 'F' || peek() == 'l' || peek() == 'L' ||
        peek() == 'u' || peek() == 'U') {
        advance();
    }
    
    return makeToken(isFloat ? TokenType::FLOAT_LITERAL : TokenType::INTEGER_LITERAL);
}

// 处理字符串字面量
std::shared_ptr<Token> Lexer::string() {
    while (peek() != '"' && !isAtEnd()) {
        if (peek() == '\\' && peekNext() == '"') {
            // 转义双引号
            advance();
        }
        advance();
    }
    
    if (isAtEnd()) {
        return errorToken("未闭合的字符串");
    }
    
    // 消费闭合的双引号
    advance();
    
    return makeToken(TokenType::STRING_LITERAL);
}

// 处理字符字面量
std::shared_ptr<Token> Lexer::character() {
    if (isAtEnd()) {
        return errorToken("未闭合的字符字面量");
    }
    
    if (peek() == '\\') {
        // 处理转义字符
        advance();
        if (isAtEnd()) {
            return errorToken("未闭合的字符字面量");
        }
        advance();
    } else {
        advance();
    }
    
    if (peek() != '\'') {
        return errorToken("未闭合的字符字面量");
    }
    
    // 消费闭合的单引号
    advance();
    
    return makeToken(TokenType::CHAR_LITERAL);
}

// 跳过空白字符
void Lexer::skipWhitespace() {
    while (true) {
        char c = peek();
        switch (c) {
            case ' ':
            case '\r':
            case '\t':
                advance();
                break;
            case '\n':
                advance();
                break;
            default:
                return;
        }
    }
}

// 跳过多行注释
void Lexer::skipComment() {
    int nesting = 1;
    
    while (nesting > 0 && !isAtEnd()) {
        if (peek() == '/' && peekNext() == '*') {
            advance();
            advance();
            nesting++;
        } else if (peek() == '*' && peekNext() == '/') {
            advance();
            advance();
            nesting--;
        } else {
            advance();
        }
    }
    
    if (nesting > 0) {
        errorToken("未闭合的多行注释");
    }
}

// 创建词法单元
std::shared_ptr<Token> Lexer::makeToken(TokenType type) {
    std::string lexeme = source.substr(start, current - start);
    return std::make_shared<Token>(type, lexeme, line, column - (current - start));
}

// 创建错误词法单元
std::shared_ptr<Token> Lexer::errorToken(const std::string& message) {
    return std::make_shared<Token>(TokenType::ERROR, message, line, column);
}

// 判断字符是否是数字
bool Lexer::isDigit(char c) const {
    return c >= '0' && c <= '9';
}

// 判断字符是否是字母或下划线
bool Lexer::isAlpha(char c) const {
    return (c >= 'a' && c <= 'z') ||
           (c >= 'A' && c <= 'Z') ||
           c == '_';
}

// 判断字符是否是字母、数字或下划线
bool Lexer::isAlphaNumeric(char c) const {
    return isAlpha(c) || isDigit(c);
}

} // namespace starry