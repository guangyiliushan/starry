#include "starry/Parser.h"
#include <iostream>
#include <sstream>

namespace starry {

Parser::Parser(std::unique_ptr<Lexer> lexer) 
    : lexer_(std::move(lexer)), current_index_(0) {
    // 预先读取所有词法单元
    Token token;
    do {
        token = lexer_->nextToken();
        tokens_.push_back(std::make_shared<Token>(token));
    } while (token.getType() != TokenType::EOF_TOKEN);
    
    // 设置当前词法单元
    if (!tokens_.empty()) {
        current_token_ = *tokens_[0];
    }
}

std::unique_ptr<ProgramNode> Parser::parseProgram() {
    auto program = std::make_unique<ProgramNode>();
    
    while (current_token_.getType() != TokenType::EOF_TOKEN) {
        try {
            auto stmt = parseStatement();
            if (stmt) {
                program->addStatement(std::move(stmt));
            }
        } catch (const std::exception& e) {
            error("解析语句时发生错误: " + std::string(e.what()));
            synchronize();
        }
    }
    
    return program;
}

void Parser::advance() {
    if (current_index_ < tokens_.size() - 1) {
        current_index_++;
        current_token_ = *tokens_[current_index_];
    }
}

bool Parser::match(TokenType type) {
    if (current_token_.getType() == type) {
        advance();
        return true;
    }
    return false;
}

bool Parser::consume(TokenType type, const std::string& message) {
    if (current_token_.getType() == type) {
        advance();
        return true;
    }
    error(message);
    return false;
}

void Parser::error(const std::string& message) {
    std::ostringstream oss;
    oss << "第" << current_token_.getLine() << "行，第" << current_token_.getColumn() 
        << "列: " << message;
    errors_.push_back(oss.str());
}

std::unique_ptr<StatementNode> Parser::parseStatement() {
    switch (current_token_.getType()) {
        case TokenType::VAR:
        case TokenType::CONST:
            return parseVariableDeclaration();
        case TokenType::FUNCTION:
            return parseFunctionDeclaration();
        case TokenType::CLASS:
            return parseClassDeclaration();
        case TokenType::IF:
            return parseIfStatement();
        case TokenType::WHILE:
            return parseWhileStatement();
        case TokenType::FOR:
            return parseForStatement();
        case TokenType::RETURN:
            return parseReturnStatement();
        case TokenType::BREAK:
            return parseBreakStatement();
        case TokenType::CONTINUE:
            return parseContinueStatement();
        case TokenType::LEFT_BRACE:
            return parseBlockStatement();
        default:
            return parseExpressionStatement();
    }
}

std::unique_ptr<StatementNode> Parser::parseVariableDeclaration() {
    bool isConst = match(TokenType::CONST);
    if (!isConst) {
        consume(TokenType::VAR, "期望 'var' 关键字");
    }
    
    if (current_token_.getType() != TokenType::IDENTIFIER) {
        error("期望变量名");
        return nullptr;
    }
    
    std::string name = current_token_.getValue();
    advance();
    
    std::string type;
    if (match(TokenType::COLON)) {
        type = parseType();
    }
    
    std::unique_ptr<ExpressionNode> initializer = nullptr;
    if (match(TokenType::ASSIGN)) {
        initializer = parseExpression();
    }
    
    consume(TokenType::SEMICOLON, "期望 ';'");
    
    return std::make_unique<VariableDeclarationNode>(name, type, std::move(initializer), isConst);
}

std::unique_ptr<StatementNode> Parser::parseFunctionDeclaration() {
    consume(TokenType::FUNCTION, "期望 'function' 关键字");
    
    if (current_token_.getType() != TokenType::IDENTIFIER) {
        error("期望函数名");
        return nullptr;
    }
    
    std::string name = current_token_.getValue();
    advance();
    
    consume(TokenType::LEFT_PAREN, "期望 '('");
    auto parameters = parseParameterList();
    consume(TokenType::RIGHT_PAREN, "期望 ')'");
    
    std::string returnType;
    if (match(TokenType::ARROW)) {
        returnType = parseType();
    }
    
    auto body = parseBlockStatement();
    
    return std::make_unique<FunctionDeclarationNode>(name, std::move(parameters), 
                                                   returnType, std::move(body));
}

std::unique_ptr<StatementNode> Parser::parseClassDeclaration() {
    consume(TokenType::CLASS, "期望 'class' 关键字");
    
    if (current_token_.getType() != TokenType::IDENTIFIER) {
        error("期望类名");
        return nullptr;
    }
    
    std::string name = current_token_.getValue();
    advance();
    
    std::string superclass;
    if (match(TokenType::EXTENDS)) {
        if (current_token_.getType() != TokenType::IDENTIFIER) {
            error("期望父类名");
            return nullptr;
        }
        superclass = current_token_.getValue();
        advance();
    }
    
    consume(TokenType::LEFT_BRACE, "期望 '{'");
    
    std::vector<std::unique_ptr<StatementNode>> members;
    while (current_token_.getType() != TokenType::RIGHT_BRACE && 
           current_token_.getType() != TokenType::EOF_TOKEN) {
        auto member = parseStatement();
        if (member) {
            members.push_back(std::move(member));
        }
    }
    
    consume(TokenType::RIGHT_BRACE, "期望 '}'");
    
    return std::make_unique<ClassDeclarationNode>(name, superclass, std::move(members));
}

std::unique_ptr<StatementNode> Parser::parseIfStatement() {
    consume(TokenType::IF, "期望 'if' 关键字");
    consume(TokenType::LEFT_PAREN, "期望 '('");
    
    auto condition = parseExpression();
    
    consume(TokenType::RIGHT_PAREN, "期望 ')'");
    
    auto thenBranch = parseStatement();
    
    std::unique_ptr<StatementNode> elseBranch = nullptr;
    if (match(TokenType::ELSE)) {
        elseBranch = parseStatement();
    }
    
    return std::make_unique<IfStatementNode>(std::move(condition), 
                                           std::move(thenBranch), 
                                           std::move(elseBranch));
}

std::unique_ptr<StatementNode> Parser::parseWhileStatement() {
    consume(TokenType::WHILE, "期望 'while' 关键字");
    consume(TokenType::LEFT_PAREN, "期望 '('");
    
    auto condition = parseExpression();
    
    consume(TokenType::RIGHT_PAREN, "期望 ')'");
    
    auto body = parseStatement();
    
    return std::make_unique<WhileStatementNode>(std::move(condition), std::move(body));
}

std::unique_ptr<StatementNode> Parser::parseForStatement() {
    consume(TokenType::FOR, "期望 'for' 关键字");
    consume(TokenType::LEFT_PAREN, "期望 '('");
    
    std::unique_ptr<StatementNode> initializer = nullptr;
    if (match(TokenType::SEMICOLON)) {
        // 空初始化器
    } else if (current_token_.getType() == TokenType::VAR) {
        initializer = parseVariableDeclaration();
    } else {
        initializer = parseExpressionStatement();
    }
    
    std::unique_ptr<ExpressionNode> condition = nullptr;
    if (current_token_.getType() != TokenType::SEMICOLON) {
        condition = parseExpression();
    }
    consume(TokenType::SEMICOLON, "期望 ';'");
    
    std::unique_ptr<ExpressionNode> increment = nullptr;
    if (current_token_.getType() != TokenType::RIGHT_PAREN) {
        increment = parseExpression();
    }
    consume(TokenType::RIGHT_PAREN, "期望 ')'");
    
    auto body = parseStatement();
    
    return std::make_unique<ForStatementNode>(std::move(initializer), 
                                            std::move(condition), 
                                            std::move(increment), 
                                            std::move(body));
}

std::unique_ptr<StatementNode> Parser::parseReturnStatement() {
    consume(TokenType::RETURN, "期望 'return' 关键字");
    
    std::unique_ptr<ExpressionNode> value = nullptr;
    if (current_token_.getType() != TokenType::SEMICOLON) {
        value = parseExpression();
    }
    
    consume(TokenType::SEMICOLON, "期望 ';'");
    
    return std::make_unique<ReturnStatementNode>(std::move(value));
}

std::unique_ptr<StatementNode> Parser::parseBreakStatement() {
    consume(TokenType::BREAK, "期望 'break' 关键字");
    consume(TokenType::SEMICOLON, "期望 ';'");
    
    return std::make_unique<BreakStatementNode>();
}

std::unique_ptr<StatementNode> Parser::parseContinueStatement() {
    consume(TokenType::CONTINUE, "期望 'continue' 关键字");
    consume(TokenType::SEMICOLON, "期望 ';'");
    
    return std::make_unique<ContinueStatementNode>();
}

std::unique_ptr<StatementNode> Parser::parseBlockStatement() {
    consume(TokenType::LEFT_BRACE, "期望 '{'");
    
    std::vector<std::unique_ptr<StatementNode>> statements;
    while (current_token_.getType() != TokenType::RIGHT_BRACE && 
           current_token_.getType() != TokenType::EOF_TOKEN) {
        auto stmt = parseStatement();
        if (stmt) {
            statements.push_back(std::move(stmt));
        }
    }
    
    consume(TokenType::RIGHT_BRACE, "期望 '}'");
    
    return std::make_unique<BlockStatementNode>(std::move(statements));
}

std::unique_ptr<StatementNode> Parser::parseExpressionStatement() {
    auto expr = parseExpression();
    consume(TokenType::SEMICOLON, "期望 ';'");
    
    return std::make_unique<ExpressionStatementNode>(std::move(expr));
}

std::unique_ptr<ExpressionNode> Parser::parseExpression() {
    return parseAssignment();
}

std::unique_ptr<ExpressionNode> Parser::parseAssignment() {
    auto expr = parseLogicalOr();
    
    if (match(TokenType::ASSIGN)) {
        auto value = parseAssignment();
        return std::make_unique<AssignmentExpressionNode>(std::move(expr), std::move(value));
    }
    
    return expr;
}

std::unique_ptr<ExpressionNode> Parser::parseLogicalOr() {
    auto expr = parseLogicalAnd();
    
    while (match(TokenType::OR)) {
        auto right = parseLogicalAnd();
        expr = std::make_unique<BinaryExpressionNode>(std::move(expr), "||", std::move(right));
    }
    
    return expr;
}

std::unique_ptr<ExpressionNode> Parser::parseLogicalAnd() {
    auto expr = parseEquality();
    
    while (match(TokenType::AND)) {
        auto right = parseEquality();
        expr = std::make_unique<BinaryExpressionNode>(std::move(expr), "&&", std::move(right));
    }
    
    return expr;
}

std::unique_ptr<ExpressionNode> Parser::parseEquality() {
    auto expr = parseComparison();
    
    while (current_token_.getType() == TokenType::EQUAL || 
           current_token_.getType() == TokenType::NOT_EQUAL) {
        std::string op = current_token_.getValue();
        advance();
        auto right = parseComparison();
        expr = std::make_unique<BinaryExpressionNode>(std::move(expr), op, std::move(right));
    }
    
    return expr;
}

std::unique_ptr<ExpressionNode> Parser::parseComparison() {
    auto expr = parseTerm();
    
    while (current_token_.getType() == TokenType::GREATER ||
           current_token_.getType() == TokenType::GREATER_EQUAL ||
           current_token_.getType() == TokenType::LESS ||
           current_token_.getType() == TokenType::LESS_EQUAL) {
        std::string op = current_token_.getValue();
        advance();
        auto right = parseTerm();
        expr = std::make_unique<BinaryExpressionNode>(std::move(expr), op, std::move(right));
    }
    
    return expr;
}

std::unique_ptr<ExpressionNode> Parser::parseTerm() {
    auto expr = parseFactor();
    
    while (current_token_.getType() == TokenType::PLUS || 
           current_token_.getType() == TokenType::MINUS) {
        std::string op = current_token_.getValue();
        advance();
        auto right = parseFactor();
        expr = std::make_unique<BinaryExpressionNode>(std::move(expr), op, std::move(right));
    }
    
    return expr;
}

std::unique_ptr<ExpressionNode> Parser::parseFactor() {
    auto expr = parseUnary();
    
    while (current_token_.getType() == TokenType::MULTIPLY || 
           current_token_.getType() == TokenType::DIVIDE ||
           current_token_.getType() == TokenType::MODULO) {
        std::string op = current_token_.getValue();
        advance();
        auto right = parseUnary();
        expr = std::make_unique<BinaryExpressionNode>(std::move(expr), op, std::move(right));
    }
    
    return expr;
}

std::unique_ptr<ExpressionNode> Parser::parseUnary() {
    if (current_token_.getType() == TokenType::NOT || 
        current_token_.getType() == TokenType::MINUS) {
        std::string op = current_token_.getValue();
        advance();
        auto expr = parseUnary();
        return std::make_unique<UnaryExpressionNode>(op, std::move(expr));
    }
    
    return parseCall();
}

std::unique_ptr<ExpressionNode> Parser::parseCall() {
    auto expr = parsePrimary();
    
    while (true) {
        if (match(TokenType::LEFT_PAREN)) {
            auto arguments = parseArgumentList();
            consume(TokenType::RIGHT_PAREN, "期望 ')'");
            expr = std::make_unique<CallExpressionNode>(std::move(expr), std::move(arguments));
        } else if (match(TokenType::DOT)) {
            if (current_token_.getType() != TokenType::IDENTIFIER) {
                error("期望属性名");
                break;
            }
            std::string name = current_token_.getValue();
            advance();
            expr = std::make_unique<MemberExpressionNode>(std::move(expr), name);
        } else if (match(TokenType::LEFT_BRACKET)) {
            auto index = parseExpression();
            consume(TokenType::RIGHT_BRACKET, "期望 ']'");
            expr = std::make_unique<IndexExpressionNode>(std::move(expr), std::move(index));
        } else {
            break;
        }
    }
    
    return expr;
}

std::unique_ptr<ExpressionNode> Parser::parsePrimary() {
    switch (current_token_.getType()) {
        case TokenType::TRUE:
            advance();
            return std::make_unique<LiteralExpressionNode>(true);
            
        case TokenType::FALSE:
            advance();
            return std::make_unique<LiteralExpressionNode>(false);
            
        case TokenType::NULL_LITERAL:
            advance();
            return std::make_unique<LiteralExpressionNode>();
            
        case TokenType::NUMBER:
            {
                std::string value = current_token_.getValue();
                advance();
                return std::make_unique<LiteralExpressionNode>(std::stod(value));
            }
            
        case TokenType::STRING:
            {
                std::string value = current_token_.getValue();
                advance();
                return std::make_unique<LiteralExpressionNode>(value);
            }
            
        case TokenType::IDENTIFIER:
            {
                std::string name = current_token_.getValue();
                advance();
                return std::make_unique<IdentifierExpressionNode>(name);
            }
            
        case TokenType::LEFT_PAREN:
            {
                advance();
                auto expr = parseExpression();
                consume(TokenType::RIGHT_PAREN, "期望 ')'");
                return expr;
            }
            
        default:
            error("期望表达式");
            return nullptr;
    }
}

std::unique_ptr<ParameterNode> Parser::parseParameter() {
    if (current_token_.getType() != TokenType::IDENTIFIER) {
        error("期望参数名");
        return nullptr;
    }
    
    std::string name = current_token_.getValue();
    advance();
    
    std::string type;
    if (match(TokenType::COLON)) {
        type = parseType();
    }
    
    std::unique_ptr<ExpressionNode> defaultValue = nullptr;
    if (match(TokenType::ASSIGN)) {
        defaultValue = parseExpression();
    }
    
    return std::make_unique<ParameterNode>(name, type, std::move(defaultValue));
}

std::vector<std::unique_ptr<ParameterNode>> Parser::parseParameterList() {
    std::vector<std::unique_ptr<ParameterNode>> parameters;
    
    if (current_token_.getType() != TokenType::RIGHT_PAREN) {
        do {
            auto param = parseParameter();
            if (param) {
                parameters.push_back(std::move(param));
            }
        } while (match(TokenType::COMMA));
    }
    
    return parameters;
}

std::vector<std::unique_ptr<ExpressionNode>> Parser::parseArgumentList() {
    std::vector<std::unique_ptr<ExpressionNode>> arguments;
    
    if (current_token_.getType() != TokenType::RIGHT_PAREN) {
        do {
            auto arg = parseExpression();
            if (arg) {
                arguments.push_back(std::move(arg));
            }
        } while (match(TokenType::COMMA));
    }
    
    return arguments;
}

std::string Parser::parseType() {
    if (current_token_.getType() != TokenType::IDENTIFIER) {
        error("期望类型名");
        return "";
    }
    
    std::string type = current_token_.getValue();
    advance();
    
    // 处理泛型类型，如 Array<int>
    if (match(TokenType::LESS)) {
        type += "<";
        do {
            type += parseType();
        } while (match(TokenType::COMMA) && (type += ",", true));
        
        consume(TokenType::GREATER, "期望 '>'");
        type += ">";
    }
    
    return type;
}

void Parser::synchronize() {
    advance();
    
    while (current_token_.getType() != TokenType::EOF_TOKEN) {
        if (tokens_[current_index_ - 1]->getType() == TokenType::SEMICOLON) {
            return;
        }
        
        switch (current_token_.getType()) {
            case TokenType::CLASS:
            case TokenType::FUNCTION:
            case TokenType::VAR:
            case TokenType::FOR:
            case TokenType::IF:
            case TokenType::WHILE:
            case TokenType::RETURN:
                return;
            default:
                break;
        }
        
        advance();
    }
}

} // namespace starry