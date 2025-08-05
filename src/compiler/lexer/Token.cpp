#include "starry/Lexer.h"
#include <sstream>
#include <unordered_map>

namespace starry {

// 将TokenType映射为字符串的辅助函数
std::string tokenTypeToString(TokenType type) {
    static const std::unordered_map<TokenType, std::string> tokenNames = {
        {TokenType::CLASS, "CLASS"},
        {TokenType::STRUCT, "STRUCT"},
        {TokenType::ENUM, "ENUM"},
        {TokenType::UNION, "UNION"},
        {TokenType::TYPEDEF, "TYPEDEF"},
        {TokenType::USING, "USING"},
        {TokenType::IF, "IF"},
        {TokenType::ELSE, "ELSE"},
        {TokenType::SWITCH, "SWITCH"},
        {TokenType::CASE, "CASE"},
        {TokenType::DEFAULT, "DEFAULT"},
        {TokenType::FOR, "FOR"},
        {TokenType::WHILE, "WHILE"},
        {TokenType::DO, "DO"},
        {TokenType::BREAK, "BREAK"},
        {TokenType::CONTINUE, "CONTINUE"},
        {TokenType::RETURN, "RETURN"},
        {TokenType::GOTO, "GOTO"},
        {TokenType::TRY, "TRY"},
        {TokenType::CATCH, "CATCH"},
        {TokenType::THROW, "THROW"},
        {TokenType::CONST, "CONST"},
        {TokenType::VOLATILE, "VOLATILE"},
        {TokenType::STATIC, "STATIC"},
        {TokenType::EXTERN, "EXTERN"},
        {TokenType::INLINE, "INLINE"},
        {TokenType::VIRTUAL, "VIRTUAL"},
        {TokenType::EXPLICIT, "EXPLICIT"},
        {TokenType::FRIEND, "FRIEND"},
        {TokenType::MUTABLE, "MUTABLE"},
        {TokenType::PUBLIC, "PUBLIC"},
        {TokenType::PRIVATE, "PRIVATE"},
        {TokenType::PROTECTED, "PROTECTED"},
        {TokenType::NEW, "NEW"},
        {TokenType::DELETE, "DELETE"},
        {TokenType::SIZEOF, "SIZEOF"},
        {TokenType::TEMPLATE, "TEMPLATE"},
        {TokenType::TYPENAME, "TYPENAME"},
        {TokenType::NAMESPACE, "NAMESPACE"},
        {TokenType::TRUE, "TRUE"},
        {TokenType::FALSE, "FALSE"},
        {TokenType::NULL_LITERAL, "NULL_LITERAL"},
        {TokenType::VAR, "VAR"},
        {TokenType::VAL, "VAL"},
        {TokenType::IS, "IS"},
        {TokenType::AS, "AS"},
        {TokenType::AS_SAFE, "AS_SAFE"},
        {TokenType::EXTENSION, "EXTENSION"},
        {TokenType::IDENTIFIER, "IDENTIFIER"},
        {TokenType::INTEGER_LITERAL, "INTEGER_LITERAL"},
        {TokenType::FLOAT_LITERAL, "FLOAT_LITERAL"},
        {TokenType::STRING_LITERAL, "STRING_LITERAL"},
        {TokenType::CHAR_LITERAL, "CHAR_LITERAL"},
        {TokenType::PLUS, "PLUS"},
        {TokenType::MINUS, "MINUS"},
        {TokenType::STAR, "STAR"},
        {TokenType::SLASH, "SLASH"},
        {TokenType::PERCENT, "PERCENT"},
        {TokenType::PLUS_EQUAL, "PLUS_EQUAL"},
        {TokenType::MINUS_EQUAL, "MINUS_EQUAL"},
        {TokenType::STAR_EQUAL, "STAR_EQUAL"},
        {TokenType::SLASH_EQUAL, "SLASH_EQUAL"},
        {TokenType::PERCENT_EQUAL, "PERCENT_EQUAL"},
        {TokenType::INCREMENT, "INCREMENT"},
        {TokenType::DECREMENT, "DECREMENT"},
        {TokenType::AND, "AND"},
        {TokenType::OR, "OR"},
        {TokenType::NOT, "NOT"},
        {TokenType::EQUAL_EQUAL, "EQUAL_EQUAL"},
        {TokenType::NOT_EQUAL, "NOT_EQUAL"},
        {TokenType::EQUAL_EQUAL_EQUAL, "EQUAL_EQUAL_EQUAL"},
        {TokenType::NOT_EQUAL_EQUAL, "NOT_EQUAL_EQUAL"},
        {TokenType::LESS, "LESS"},
        {TokenType::GREATER, "GREATER"},
        {TokenType::LESS_EQUAL, "LESS_EQUAL"},
        {TokenType::GREATER_EQUAL, "GREATER_EQUAL"},
        {TokenType::BIT_AND, "BIT_AND"},
        {TokenType::BIT_OR, "BIT_OR"},
        {TokenType::BIT_XOR, "BIT_XOR"},
        {TokenType::BIT_NOT, "BIT_NOT"},
        {TokenType::BIT_AND_EQUAL, "BIT_AND_EQUAL"},
        {TokenType::BIT_OR_EQUAL, "BIT_OR_EQUAL"},
        {TokenType::BIT_XOR_EQUAL, "BIT_XOR_EQUAL"},
        {TokenType::LEFT_SHIFT, "LEFT_SHIFT"},
        {TokenType::RIGHT_SHIFT, "RIGHT_SHIFT"},
        {TokenType::LEFT_SHIFT_EQUAL, "LEFT_SHIFT_EQUAL"},
        {TokenType::RIGHT_SHIFT_EQUAL, "RIGHT_SHIFT_EQUAL"},
        {TokenType::SAFE_DOT, "SAFE_DOT"},
        {TokenType::ELVIS, "ELVIS"},
        {TokenType::NOT_NULL, "NOT_NULL"},
        {TokenType::RANGE, "RANGE"},
        {TokenType::RANGE_EXCLUSIVE, "RANGE_EXCLUSIVE"},
        {TokenType::RANGE_INCLUSIVE, "RANGE_INCLUSIVE"},
        {TokenType::SCOPE, "SCOPE"},
        {TokenType::DOT, "DOT"},
        {TokenType::QUESTION, "QUESTION"},
        {TokenType::EQUAL, "EQUAL"},
        {TokenType::ARROW, "ARROW"},
        {TokenType::FAT_ARROW, "FAT_ARROW"},
        {TokenType::AT, "AT"},
        {TokenType::COLON, "COLON"},
        {TokenType::SEMICOLON, "SEMICOLON"},
        {TokenType::DOLLAR, "DOLLAR"},
        {TokenType::UNDERSCORE, "UNDERSCORE"},
        {TokenType::LEFT_PAREN, "LEFT_PAREN"},
        {TokenType::RIGHT_PAREN, "RIGHT_PAREN"},
        {TokenType::LEFT_BRACE, "LEFT_BRACE"},
        {TokenType::RIGHT_BRACE, "RIGHT_BRACE"},
        {TokenType::LEFT_BRACKET, "LEFT_BRACKET"},
        {TokenType::RIGHT_BRACKET, "RIGHT_BRACKET"},
        {TokenType::COMMA, "COMMA"},
        {TokenType::END_OF_FILE, "END_OF_FILE"},
        {TokenType::ERROR, "ERROR"}
    };
    
    auto it = tokenNames.find(type);
    if (it != tokenNames.end()) {
        return it->second;
    }
    return "UNKNOWN";
}

// Token类的toString方法实现
std::string Token::toString() const {
    std::stringstream ss;
    ss << "Token(type=" << tokenTypeToString(type) 
       << ", lexeme='" << lexeme 
       << "', line=" << line 
       << ", column=" << column << ")";
    return ss.str();
}

} // namespace starry