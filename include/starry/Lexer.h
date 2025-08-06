#ifndef STARRY_LEXER_H
#define STARRY_LEXER_H

#include <string>
#include <vector>
#include <memory>
#include <unordered_map>

namespace starry {

// 词法单元类型枚举
enum class TokenType {
    // 关键字
    CLASS, STRUCT, ENUM, UNION, TYPEDEF, USING,
    IF, ELSE, SWITCH, CASE, DEFAULT, FOR, WHILE, DO,
    BREAK, CONTINUE, RETURN, GOTO,
    TRY, CATCH, THROW,
    CONST, VOLATILE, STATIC, EXTERN, INLINE, VIRTUAL, EXPLICIT, FRIEND, MUTABLE,
    PUBLIC, PRIVATE, PROTECTED,
    NEW, DELETE, SIZEOF,
    TEMPLATE, TYPENAME,
    NAMESPACE,
    TRUE, FALSE,
    NULL_LITERAL,
    VAR, VAL,
    IS, AS, AS_SAFE,
    EXTENSION,
    
    // 标识符
    IDENTIFIER,
    
    // 字面量
    INTEGER_LITERAL,
    FLOAT_LITERAL,
    STRING_LITERAL,
    CHAR_LITERAL,
    
    // 运算符
    PLUS, MINUS, STAR, SLASH, PERCENT,           // + - * / %
    PLUS_EQUAL, MINUS_EQUAL, STAR_EQUAL, SLASH_EQUAL, PERCENT_EQUAL, // += -= *= /= %=
    INCREMENT, DECREMENT,                        // ++ --
    AND, OR, NOT,                                // && || !
    EQUAL_EQUAL, NOT_EQUAL, EQUAL_EQUAL_EQUAL, NOT_EQUAL_EQUAL, // == != === !==
    LESS, GREATER, LESS_EQUAL, GREATER_EQUAL,    // < > <= >=
    BIT_AND, BIT_OR, BIT_XOR, BIT_NOT,           // & | ^ ~
    BIT_AND_EQUAL, BIT_OR_EQUAL, BIT_XOR_EQUAL,  // &= |= ^=
    LEFT_SHIFT, RIGHT_SHIFT,                     // << >>
    LEFT_SHIFT_EQUAL, RIGHT_SHIFT_EQUAL,         // <<= >>=
    SAFE_DOT, ELVIS, NOT_NULL,                   // ?. ?: !!
    RANGE, RANGE_EXCLUSIVE, RANGE_INCLUSIVE,     // .. ..< ..=
    SCOPE, DOT, QUESTION,                        // :: . ?
    EQUAL, ARROW, FAT_ARROW,                     // = -> =>
    AT, COLON, SEMICOLON, DOLLAR, UNDERSCORE,    // @ : ; $ _
    
    // 分隔符
    LEFT_PAREN, RIGHT_PAREN,                     // ( )
    LEFT_BRACE, RIGHT_BRACE,                     // { }
    LEFT_BRACKET, RIGHT_BRACKET,                 // [ ]
    COMMA,                                       // ,
    
    // 特殊标记
    END_OF_FILE,
    ERROR
};

// 词法单元类
class Token {
public:
    Token(TokenType type, const std::string& lexeme, int line, int column)
        : type(type), lexeme(lexeme), line(line), column(column) {}
    
    TokenType getType() const { return type; }
    const std::string& getLexeme() const { return lexeme; }
    int getLine() const { return line; }
    int getColumn() const { return column; }
    
    std::string toString() const;
    
private:
    TokenType type;
    std::string lexeme;
    int line;
    int column;
};

// 词法分析器类
class Lexer {
public:
    explicit Lexer(const std::string& source);
    
    std::vector<std::shared_ptr<Token>> tokenize();
    
private:
    std::string source;
    int start = 0;
    int current = 0;
    int line = 1;
    int column = 1;
    
    static std::unordered_map<std::string, TokenType> keywords;
    
    bool isAtEnd() const;
    char advance();
    char peek() const;
    char peekNext() const;
    bool match(char expected);
    
    std::shared_ptr<Token> scanToken();
    std::shared_ptr<Token> identifier();
    std::shared_ptr<Token> number();
    std::shared_ptr<Token> string();
    std::shared_ptr<Token> character();
    
    void skipWhitespace();
    void skipComment();
    
    std::shared_ptr<Token> makeToken(TokenType type);
    std::shared_ptr<Token> errorToken(const std::string& message);
    
    bool isDigit(char c) const;
    bool isAlpha(char c) const;
    bool isAlphaNumeric(char c) const;
};

} // namespace starry

#endif // STARRY_LEXER_H