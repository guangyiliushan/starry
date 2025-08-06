#include "starry/AST.h"
#include <iostream>
#include <memory>

namespace starry {
namespace ast {

// 基础语句类实现
Statement::Statement(StatementType type) : type_(type) {}

Statement::~Statement() = default;

StatementType Statement::getType() const {
    return type_;
}

// 表达式语句实现
ExpressionStatement::ExpressionStatement(std::unique_ptr<Expression> expr)
    : Statement(StatementType::EXPRESSION), expression_(std::move(expr)) {}

void ExpressionStatement::accept(ASTVisitor& visitor) {
    visitor.visitExpressionStatement(*this);
}

Expression* ExpressionStatement::getExpression() const {
    return expression_.get();
}

// 变量声明语句实现
VariableDeclaration::VariableDeclaration(const std::string& name, 
                                       std::unique_ptr<Expression> initializer)
    : Statement(StatementType::VARIABLE_DECLARATION), 
      name_(name), initializer_(std::move(initializer)) {}

void VariableDeclaration::accept(ASTVisitor& visitor) {
    visitor.visitVariableDeclaration(*this);
}

const std::string& VariableDeclaration::getName() const {
    return name_;
}

Expression* VariableDeclaration::getInitializer() const {
    return initializer_.get();
}

// 块语句实现
BlockStatement::BlockStatement(std::vector<std::unique_ptr<Statement>> statements)
    : Statement(StatementType::BLOCK), statements_(std::move(statements)) {}

void BlockStatement::accept(ASTVisitor& visitor) {
    visitor.visitBlockStatement(*this);
}

const std::vector<std::unique_ptr<Statement>>& BlockStatement::getStatements() const {
    return statements_;
}

void BlockStatement::addStatement(std::unique_ptr<Statement> stmt) {
    statements_.push_back(std::move(stmt));
}

// if语句实现
IfStatement::IfStatement(std::unique_ptr<Expression> condition,
                        std::unique_ptr<Statement> thenStmt,
                        std::unique_ptr<Statement> elseStmt)
    : Statement(StatementType::IF),
      condition_(std::move(condition)),
      thenStatement_(std::move(thenStmt)),
      elseStatement_(std::move(elseStmt)) {}

void IfStatement::accept(ASTVisitor& visitor) {
    visitor.visitIfStatement(*this);
}

Expression* IfStatement::getCondition() const {
    return condition_.get();
}

Statement* IfStatement::getThenStatement() const {
    return thenStatement_.get();
}

Statement* IfStatement::getElseStatement() const {
    return elseStatement_.get();
}

// while循环语句实现
WhileStatement::WhileStatement(std::unique_ptr<Expression> condition,
                              std::unique_ptr<Statement> body)
    : Statement(StatementType::WHILE),
      condition_(std::move(condition)),
      body_(std::move(body)) {}

void WhileStatement::accept(ASTVisitor& visitor) {
    visitor.visitWhileStatement(*this);
}

Expression* WhileStatement::getCondition() const {
    return condition_.get();
}

Statement* WhileStatement::getBody() const {
    return body_.get();
}

// for循环语句实现
ForStatement::ForStatement(std::unique_ptr<Statement> init,
                          std::unique_ptr<Expression> condition,
                          std::unique_ptr<Expression> update,
                          std::unique_ptr<Statement> body)
    : Statement(StatementType::FOR),
      init_(std::move(init)),
      condition_(std::move(condition)),
      update_(std::move(update)),
      body_(std::move(body)) {}

void ForStatement::accept(ASTVisitor& visitor) {
    visitor.visitForStatement(*this);
}

Statement* ForStatement::getInit() const {
    return init_.get();
}

Expression* ForStatement::getCondition() const {
    return condition_.get();
}

Expression* ForStatement::getUpdate() const {
    return update_.get();
}

Statement* ForStatement::getBody() const {
    return body_.get();
}

// return语句实现
ReturnStatement::ReturnStatement(std::unique_ptr<Expression> value)
    : Statement(StatementType::RETURN), value_(std::move(value)) {}

void ReturnStatement::accept(ASTVisitor& visitor) {
    visitor.visitReturnStatement(*this);
}

Expression* ReturnStatement::getValue() const {
    return value_.get();
}

// break语句实现
BreakStatement::BreakStatement() : Statement(StatementType::BREAK) {}

void BreakStatement::accept(ASTVisitor& visitor) {
    visitor.visitBreakStatement(*this);
}

// continue语句实现
ContinueStatement::ContinueStatement() : Statement(StatementType::CONTINUE) {}

void ContinueStatement::accept(ASTVisitor& visitor) {
    visitor.visitContinueStatement(*this);
}

// 函数声明语句实现
FunctionDeclaration::FunctionDeclaration(const std::string& name,
                                        std::vector<std::string> parameters,
                                        std::unique_ptr<Statement> body)
    : Statement(StatementType::FUNCTION_DECLARATION),
      name_(name),
      parameters_(std::move(parameters)),
      body_(std::move(body)) {}

void FunctionDeclaration::accept(ASTVisitor& visitor) {
    visitor.visitFunctionDeclaration(*this);
}

const std::string& FunctionDeclaration::getName() const {
    return name_;
}

const std::vector<std::string>& FunctionDeclaration::getParameters() const {
    return parameters_;
}

Statement* FunctionDeclaration::getBody() const {
    return body_.get();
}

} // namespace ast
} // namespace starry