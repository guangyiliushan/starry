#include "starry/semantic/TypeChecker.h"
#include "starry/AST.h"
#include <iostream>
#include <stdexcept>

namespace starry {
namespace semantic {

TypeChecker::TypeChecker() {
    // 初始化内置类型
    builtinTypes_["int"] = std::make_shared<Type>(Type::Kind::INTEGER, "int");
    builtinTypes_["float"] = std::make_shared<Type>(Type::Kind::FLOAT, "float");
    builtinTypes_["string"] = std::make_shared<Type>(Type::Kind::STRING, "string");
    builtinTypes_["bool"] = std::make_shared<Type>(Type::Kind::BOOLEAN, "bool");
    builtinTypes_["void"] = std::make_shared<Type>(Type::Kind::VOID, "void");
}

TypeChecker::~TypeChecker() = default;

void TypeChecker::checkProgram(ast::Program* program) {
    if (!program) {
        throw std::runtime_error("空程序指针");
    }
    
    // 遍历程序中的所有语句进行类型检查
    for (const auto& stmt : program->getStatements()) {
        checkStatement(stmt.get());
    }
}

std::shared_ptr<Type> TypeChecker::checkExpression(ast::Expression* expr) {
    if (!expr) {
        return nullptr;
    }
    
    switch (expr->getType()) {
        case ast::ExpressionType::LITERAL:
            return checkLiteralExpression(static_cast<ast::LiteralExpression*>(expr));
        case ast::ExpressionType::IDENTIFIER:
            return checkIdentifierExpression(static_cast<ast::IdentifierExpression*>(expr));
        case ast::ExpressionType::BINARY:
            return checkBinaryExpression(static_cast<ast::BinaryExpression*>(expr));
        case ast::ExpressionType::UNARY:
            return checkUnaryExpression(static_cast<ast::UnaryExpression*>(expr));
        case ast::ExpressionType::CALL:
            return checkCallExpression(static_cast<ast::CallExpression*>(expr));
        case ast::ExpressionType::ASSIGNMENT:
            return checkAssignmentExpression(static_cast<ast::AssignmentExpression*>(expr));
        default:
            throw std::runtime_error("未知的表达式类型");
    }
}

void TypeChecker::checkStatement(ast::Statement* stmt) {
    if (!stmt) {
        return;
    }
    
    switch (stmt->getType()) {
        case ast::StatementType::EXPRESSION:
            checkExpressionStatement(static_cast<ast::ExpressionStatement*>(stmt));
            break;
        case ast::StatementType::VARIABLE_DECLARATION:
            checkVariableDeclaration(static_cast<ast::VariableDeclaration*>(stmt));
            break;
        case ast::StatementType::BLOCK:
            checkBlockStatement(static_cast<ast::BlockStatement*>(stmt));
            break;
        case ast::StatementType::IF:
            checkIfStatement(static_cast<ast::IfStatement*>(stmt));
            break;
        case ast::StatementType::WHILE:
            checkWhileStatement(static_cast<ast::WhileStatement*>(stmt));
            break;
        case ast::StatementType::FOR:
            checkForStatement(static_cast<ast::ForStatement*>(stmt));
            break;
        case ast::StatementType::RETURN:
            checkReturnStatement(static_cast<ast::ReturnStatement*>(stmt));
            break;
        case ast::StatementType::FUNCTION_DECLARATION:
            checkFunctionDeclaration(static_cast<ast::FunctionDeclaration*>(stmt));
            break;
        default:
            // break和continue语句不需要特殊的类型检查
            break;
    }
}

std::shared_ptr<Type> TypeChecker::checkLiteralExpression(ast::LiteralExpression* expr) {
    switch (expr->getLiteralType()) {
        case ast::LiteralType::INTEGER:
            return builtinTypes_["int"];
        case ast::LiteralType::FLOAT:
            return builtinTypes_["float"];
        case ast::LiteralType::STRING:
            return builtinTypes_["string"];
        case ast::LiteralType::BOOLEAN:
            return builtinTypes_["bool"];
        default:
            throw std::runtime_error("未知的字面量类型");
    }
}

std::shared_ptr<Type> TypeChecker::checkIdentifierExpression(ast::IdentifierExpression* expr) {
    const std::string& name = expr->getName();
    
    // 在符号表中查找变量
    auto symbol = symbolTable_.lookup(name);
    if (!symbol) {
        throw std::runtime_error("未定义的标识符: " + name);
    }
    
    return symbol->getType();
}

std::shared_ptr<Type> TypeChecker::checkBinaryExpression(ast::BinaryExpression* expr) {
    auto leftType = checkExpression(expr->getLeft());
    auto rightType = checkExpression(expr->getRight());
    
    if (!leftType || !rightType) {
        throw std::runtime_error("二元表达式操作数类型检查失败");
    }
    
    // 根据操作符类型进行类型检查
    switch (expr->getOperator()) {
        case ast::BinaryOperator::ADD:
        case ast::BinaryOperator::SUBTRACT:
        case ast::BinaryOperator::MULTIPLY:
        case ast::BinaryOperator::DIVIDE:
            return checkArithmeticOperation(leftType, rightType);
        case ast::BinaryOperator::EQUAL:
        case ast::BinaryOperator::NOT_EQUAL:
        case ast::BinaryOperator::LESS:
        case ast::BinaryOperator::LESS_EQUAL:
        case ast::BinaryOperator::GREATER:
        case ast::BinaryOperator::GREATER_EQUAL:
            return checkComparisonOperation(leftType, rightType);
        case ast::BinaryOperator::LOGICAL_AND:
        case ast::BinaryOperator::LOGICAL_OR:
            return checkLogicalOperation(leftType, rightType);
        default:
            throw std::runtime_error("未知的二元操作符");
    }
}

std::shared_ptr<Type> TypeChecker::checkUnaryExpression(ast::UnaryExpression* expr) {
    auto operandType = checkExpression(expr->getOperand());
    
    if (!operandType) {
        throw std::runtime_error("一元表达式操作数类型检查失败");
    }
    
    switch (expr->getOperator()) {
        case ast::UnaryOperator::MINUS:
            if (operandType->getKind() != Type::Kind::INTEGER && 
                operandType->getKind() != Type::Kind::FLOAT) {
                throw std::runtime_error("负号操作符只能用于数值类型");
            }
            return operandType;
        case ast::UnaryOperator::LOGICAL_NOT:
            if (operandType->getKind() != Type::Kind::BOOLEAN) {
                throw std::runtime_error("逻辑非操作符只能用于布尔类型");
            }
            return builtinTypes_["bool"];
        default:
            throw std::runtime_error("未知的一元操作符");
    }
}

std::shared_ptr<Type> TypeChecker::checkCallExpression(ast::CallExpression* expr) {
    const std::string& functionName = expr->getFunctionName();
    
    // 在符号表中查找函数
    auto symbol = symbolTable_.lookup(functionName);
    if (!symbol || symbol->getSymbolType() != Symbol::Type::FUNCTION) {
        throw std::runtime_error("未定义的函数: " + functionName);
    }
    
    // 检查参数类型
    const auto& arguments = expr->getArguments();
    // TODO: 实现参数类型检查
    
    return symbol->getType();
}

std::shared_ptr<Type> TypeChecker::checkAssignmentExpression(ast::AssignmentExpression* expr) {
    auto valueType = checkExpression(expr->getValue());
    auto targetType = checkExpression(expr->getTarget());
    
    if (!isTypeCompatible(targetType, valueType)) {
        throw std::runtime_error("赋值类型不兼容");
    }
    
    return targetType;
}

void TypeChecker::checkExpressionStatement(ast::ExpressionStatement* stmt) {
    checkExpression(stmt->getExpression());
}

void TypeChecker::checkVariableDeclaration(ast::VariableDeclaration* stmt) {
    const std::string& name = stmt->getName();
    
    // 检查是否已经声明
    if (symbolTable_.lookupInCurrentScope(name)) {
        throw std::runtime_error("变量重复声明: " + name);
    }
    
    std::shared_ptr<Type> type;
    if (stmt->getInitializer()) {
        type = checkExpression(stmt->getInitializer());
    } else {
        // 如果没有初始化器，需要显式类型声明
        // 这里简化处理，默认为int类型
        type = builtinTypes_["int"];
    }
    
    // 添加到符号表
    auto symbol = std::make_shared<Symbol>(name, Symbol::Type::VARIABLE, type);
    symbolTable_.define(name, symbol);
}

void TypeChecker::checkBlockStatement(ast::BlockStatement* stmt) {
    // 进入新的作用域
    symbolTable_.enterScope();
    
    // 检查块中的所有语句
    for (const auto& statement : stmt->getStatements()) {
        checkStatement(statement.get());
    }
    
    // 退出作用域
    symbolTable_.exitScope();
}

void TypeChecker::checkIfStatement(ast::IfStatement* stmt) {
    auto conditionType = checkExpression(stmt->getCondition());
    
    if (conditionType->getKind() != Type::Kind::BOOLEAN) {
        throw std::runtime_error("if语句条件必须是布尔类型");
    }
    
    checkStatement(stmt->getThenStatement());
    
    if (stmt->getElseStatement()) {
        checkStatement(stmt->getElseStatement());
    }
}

void TypeChecker::checkWhileStatement(ast::WhileStatement* stmt) {
    auto conditionType = checkExpression(stmt->getCondition());
    
    if (conditionType->getKind() != Type::Kind::BOOLEAN) {
        throw std::runtime_error("while语句条件必须是布尔类型");
    }
    
    checkStatement(stmt->getBody());
}

void TypeChecker::checkForStatement(ast::ForStatement* stmt) {
    // 进入新的作用域
    symbolTable_.enterScope();
    
    if (stmt->getInit()) {
        checkStatement(stmt->getInit());
    }
    
    if (stmt->getCondition()) {
        auto conditionType = checkExpression(stmt->getCondition());
        if (conditionType->getKind() != Type::Kind::BOOLEAN) {
            throw std::runtime_error("for语句条件必须是布尔类型");
        }
    }
    
    if (stmt->getUpdate()) {
        checkExpression(stmt->getUpdate());
    }
    
    checkStatement(stmt->getBody());
    
    // 退出作用域
    symbolTable_.exitScope();
}

void TypeChecker::checkReturnStatement(ast::ReturnStatement* stmt) {
    if (stmt->getValue()) {
        auto returnType = checkExpression(stmt->getValue());
        // TODO: 检查返回类型是否与函数声明匹配
    }
}

void TypeChecker::checkFunctionDeclaration(ast::FunctionDeclaration* stmt) {
    const std::string& name = stmt->getName();
    
    // 检查是否已经声明
    if (symbolTable_.lookupInCurrentScope(name)) {
        throw std::runtime_error("函数重复声明: " + name);
    }
    
    // 创建函数类型（简化处理）
    auto functionType = builtinTypes_["void"]; // 默认返回void
    
    // 添加到符号表
    auto symbol = std::make_shared<Symbol>(name, Symbol::Type::FUNCTION, functionType);
    symbolTable_.define(name, symbol);
    
    // 进入函数作用域
    symbolTable_.enterScope();
    
    // 添加参数到符号表
    for (const auto& param : stmt->getParameters()) {
        auto paramType = builtinTypes_["int"]; // 简化处理，默认为int
        auto paramSymbol = std::make_shared<Symbol>(param, Symbol::Type::VARIABLE, paramType);
        symbolTable_.define(param, paramSymbol);
    }
    
    // 检查函数体
    checkStatement(stmt->getBody());
    
    // 退出函数作用域
    symbolTable_.exitScope();
}

std::shared_ptr<Type> TypeChecker::checkArithmeticOperation(
    std::shared_ptr<Type> left, std::shared_ptr<Type> right) {
    
    if (left->getKind() == Type::Kind::INTEGER && right->getKind() == Type::Kind::INTEGER) {
        return builtinTypes_["int"];
    } else if ((left->getKind() == Type::Kind::INTEGER || left->getKind() == Type::Kind::FLOAT) &&
               (right->getKind() == Type::Kind::INTEGER || right->getKind() == Type::Kind::FLOAT)) {
        return builtinTypes_["float"];
    } else {
        throw std::runtime_error("算术操作类型不兼容");
    }
}

std::shared_ptr<Type> TypeChecker::checkComparisonOperation(
    std::shared_ptr<Type> left, std::shared_ptr<Type> right) {
    
    if (!isTypeCompatible(left, right)) {
        throw std::runtime_error("比较操作类型不兼容");
    }
    
    return builtinTypes_["bool"];
}

std::shared_ptr<Type> TypeChecker::checkLogicalOperation(
    std::shared_ptr<Type> left, std::shared_ptr<Type> right) {
    
    if (left->getKind() != Type::Kind::BOOLEAN || right->getKind() != Type::Kind::BOOLEAN) {
        throw std::runtime_error("逻辑操作只能用于布尔类型");
    }
    
    return builtinTypes_["bool"];
}

bool TypeChecker::isTypeCompatible(std::shared_ptr<Type> target, std::shared_ptr<Type> source) {
    if (!target || !source) {
        return false;
    }
    
    // 相同类型
    if (target->getKind() == source->getKind()) {
        return true;
    }
    
    // 数值类型之间的隐式转换
    if ((target->getKind() == Type::Kind::FLOAT) && 
        (source->getKind() == Type::Kind::INTEGER)) {
        return true;
    }
    
    return false;
}

} // namespace semantic
} // namespace starry