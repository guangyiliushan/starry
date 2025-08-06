#pragma once

#include "starry/semantic/SymbolTable.h"
#include "starry/semantic/Type.h"
#include <memory>
#include <unordered_map>

namespace starry {
namespace ast {
    class Program;
    class Expression;
    class Statement;
    class LiteralExpression;
    class IdentifierExpression;
    class BinaryExpression;
    class UnaryExpression;
    class CallExpression;
    class AssignmentExpression;
    class ExpressionStatement;
    class VariableDeclaration;
    class BlockStatement;
    class IfStatement;
    class WhileStatement;
    class ForStatement;
    class ReturnStatement;
    class FunctionDeclaration;
}

namespace starry {
namespace semantic {

/**
 * @brief 类型检查器类
 * 
 * 负责对AST进行语义分析和类型检查，确保程序的类型安全性
 */
class TypeChecker {
public:
    TypeChecker();
    ~TypeChecker();

    /**
     * @brief 检查整个程序的类型
     * @param program 程序AST根节点
     */
    void checkProgram(ast::Program* program);

    /**
     * @brief 检查表达式的类型
     * @param expr 表达式节点
     * @return 表达式的类型
     */
    std::shared_ptr<Type> checkExpression(ast::Expression* expr);

    /**
     * @brief 检查语句的类型
     * @param stmt 语句节点
     */
    void checkStatement(ast::Statement* stmt);

    /**
     * @brief 检查两个类型是否兼容
     * @param target 目标类型
     * @param source 源类型
     * @return 如果兼容返回true，否则返回false
     */
    bool isTypeCompatible(std::shared_ptr<Type> target, std::shared_ptr<Type> source);

private:
    // 表达式类型检查方法
    std::shared_ptr<Type> checkLiteralExpression(ast::LiteralExpression* expr);
    std::shared_ptr<Type> checkIdentifierExpression(ast::IdentifierExpression* expr);
    std::shared_ptr<Type> checkBinaryExpression(ast::BinaryExpression* expr);
    std::shared_ptr<Type> checkUnaryExpression(ast::UnaryExpression* expr);
    std::shared_ptr<Type> checkCallExpression(ast::CallExpression* expr);
    std::shared_ptr<Type> checkAssignmentExpression(ast::AssignmentExpression* expr);

    // 语句类型检查方法
    void checkExpressionStatement(ast::ExpressionStatement* stmt);
    void checkVariableDeclaration(ast::VariableDeclaration* stmt);
    void checkBlockStatement(ast::BlockStatement* stmt);
    void checkIfStatement(ast::IfStatement* stmt);
    void checkWhileStatement(ast::WhileStatement* stmt);
    void checkForStatement(ast::ForStatement* stmt);
    void checkReturnStatement(ast::ReturnStatement* stmt);
    void checkFunctionDeclaration(ast::FunctionDeclaration* stmt);

    // 操作符类型检查方法
    std::shared_ptr<Type> checkArithmeticOperation(std::shared_ptr<Type> left, std::shared_ptr<Type> right);
    std::shared_ptr<Type> checkComparisonOperation(std::shared_ptr<Type> left, std::shared_ptr<Type> right);
    std::shared_ptr<Type> checkLogicalOperation(std::shared_ptr<Type> left, std::shared_ptr<Type> right);

private:
    SymbolTable symbolTable_;                                    ///< 符号表
    std::unordered_map<std::string, std::shared_ptr<Type>> builtinTypes_; ///< 内置类型映射
};

} // namespace semantic
} // namespace starry