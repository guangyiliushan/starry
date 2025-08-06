#include "starry/ast/Visitor.h"
#include "starry/ast/Expression.h"
#include "starry/ast/Statement.h"
#include "starry/ast/Declaration.h"
#include "starry/ast/Type.h"
#include <stdexcept>

namespace starry {
namespace ast {

// BaseVisitor 实现
BaseVisitor::~BaseVisitor() = default;

void BaseVisitor::visit(ASTNode* node) {
    if (!node) return;
    node->accept(this);
}

// 默认访问方法实现
void BaseVisitor::visitExpression(Expression* expr) {
    // 默认实现：访问子节点
    for (auto& child : expr->getChildren()) {
        visit(child.get());
    }
}

void BaseVisitor::visitStatement(Statement* stmt) {
    // 默认实现：访问子节点
    for (auto& child : stmt->getChildren()) {
        visit(child.get());
    }
}

void BaseVisitor::visitDeclaration(Declaration* decl) {
    // 默认实现：访问子节点
    for (auto& child : decl->getChildren()) {
        visit(child.get());
    }
}

void BaseVisitor::visitType(Type* type) {
    // 默认实现：访问子节点
    for (auto& child : type->getChildren()) {
        visit(child.get());
    }
}

// 具体表达式访问方法
void BaseVisitor::visitBinaryExpression(BinaryExpression* expr) {
    visit(expr->getLeft());
    visit(expr->getRight());
}

void BaseVisitor::visitUnaryExpression(UnaryExpression* expr) {
    visit(expr->getOperand());
}

void BaseVisitor::visitLiteralExpression(LiteralExpression* expr) {
    // 字面量表达式没有子节点
}

void BaseVisitor::visitIdentifierExpression(IdentifierExpression* expr) {
    // 标识符表达式没有子节点
}

void BaseVisitor::visitCallExpression(CallExpression* expr) {
    visit(expr->getCallee());
    for (auto& arg : expr->getArguments()) {
        visit(arg.get());
    }
}

void BaseVisitor::visitMemberExpression(MemberExpression* expr) {
    visit(expr->getObject());
}

void BaseVisitor::visitIndexExpression(IndexExpression* expr) {
    visit(expr->getObject());
    visit(expr->getIndex());
}

void BaseVisitor::visitAssignmentExpression(AssignmentExpression* expr) {
    visit(expr->getLeft());
    visit(expr->getRight());
}

// 具体语句访问方法
void BaseVisitor::visitExpressionStatement(ExpressionStatement* stmt) {
    visit(stmt->getExpression());
}

void BaseVisitor::visitBlockStatement(BlockStatement* stmt) {
    for (auto& statement : stmt->getStatements()) {
        visit(statement.get());
    }
}

void BaseVisitor::visitIfStatement(IfStatement* stmt) {
    visit(stmt->getCondition());
    visit(stmt->getThenStatement());
    if (stmt->getElseStatement()) {
        visit(stmt->getElseStatement());
    }
}

void BaseVisitor::visitWhileStatement(WhileStatement* stmt) {
    visit(stmt->getCondition());
    visit(stmt->getBody());
}

void BaseVisitor::visitForStatement(ForStatement* stmt) {
    if (stmt->getInit()) {
        visit(stmt->getInit());
    }
    if (stmt->getCondition()) {
        visit(stmt->getCondition());
    }
    if (stmt->getUpdate()) {
        visit(stmt->getUpdate());
    }
    visit(stmt->getBody());
}

void BaseVisitor::visitReturnStatement(ReturnStatement* stmt) {
    if (stmt->getValue()) {
        visit(stmt->getValue());
    }
}

void BaseVisitor::visitBreakStatement(BreakStatement* stmt) {
    // break语句没有子节点
}

void BaseVisitor::visitContinueStatement(ContinueStatement* stmt) {
    // continue语句没有子节点
}

// 具体声明访问方法
void BaseVisitor::visitVariableDeclaration(VariableDeclaration* decl) {
    if (decl->getType()) {
        visit(decl->getType());
    }
    if (decl->getInitializer()) {
        visit(decl->getInitializer());
    }
}

void BaseVisitor::visitFunctionDeclaration(FunctionDeclaration* decl) {
    if (decl->getReturnType()) {
        visit(decl->getReturnType());
    }
    for (auto& param : decl->getParameters()) {
        visit(param.get());
    }
    if (decl->getBody()) {
        visit(decl->getBody());
    }
}

void BaseVisitor::visitClassDeclaration(ClassDeclaration* decl) {
    if (decl->getSuperClass()) {
        visit(decl->getSuperClass());
    }
    for (auto& member : decl->getMembers()) {
        visit(member.get());
    }
}

void BaseVisitor::visitStructDeclaration(StructDeclaration* decl) {
    for (auto& field : decl->getFields()) {
        visit(field.get());
    }
}

void BaseVisitor::visitEnumDeclaration(EnumDeclaration* decl) {
    for (auto& value : decl->getValues()) {
        visit(value.get());
    }
}

void BaseVisitor::visitInterfaceDeclaration(InterfaceDeclaration* decl) {
    for (auto& method : decl->getMethods()) {
        visit(method.get());
    }
}

// 具体类型访问方法
void BaseVisitor::visitBasicType(BasicType* type) {
    // 基础类型没有子节点
}

void BaseVisitor::visitArrayType(ArrayType* type) {
    visit(type->getElementType());
    if (type->getSize()) {
        visit(type->getSize());
    }
}

void BaseVisitor::visitPointerType(PointerType* type) {
    visit(type->getPointeeType());
}

void BaseVisitor::visitFunctionType(FunctionType* type) {
    visit(type->getReturnType());
    for (auto& paramType : type->getParameterTypes()) {
        visit(paramType.get());
    }
}

void BaseVisitor::visitStructType(StructType* type) {
    for (auto& fieldType : type->getFieldTypes()) {
        visit(fieldType.get());
    }
}

void BaseVisitor::visitClassType(ClassType* type) {
    // 类类型引用，通常没有子节点需要访问
}

void BaseVisitor::visitGenericType(GenericType* type) {
    for (auto& arg : type->getTypeArguments()) {
        visit(arg.get());
    }
}

// PrintVisitor 实现
PrintVisitor::PrintVisitor(std::ostream& os) : output_(os), indent_(0) {}

void PrintVisitor::visitExpression(Expression* expr) {
    printIndent();
    output_ << "Expression: " << expr->getTypeName() << std::endl;
    indent_++;
    BaseVisitor::visitExpression(expr);
    indent_--;
}

void PrintVisitor::visitStatement(Statement* stmt) {
    printIndent();
    output_ << "Statement: " << stmt->getTypeName() << std::endl;
    indent_++;
    BaseVisitor::visitStatement(stmt);
    indent_--;
}

void PrintVisitor::visitDeclaration(Declaration* decl) {
    printIndent();
    output_ << "Declaration: " << decl->getTypeName() << std::endl;
    indent_++;
    BaseVisitor::visitDeclaration(decl);
    indent_--;
}

void PrintVisitor::visitType(Type* type) {
    printIndent();
    output_ << "Type: " << type->getTypeName() << std::endl;
    indent_++;
    BaseVisitor::visitType(type);
    indent_--;
}

void PrintVisitor::printIndent() {
    for (int i = 0; i < indent_; ++i) {
        output_ << "  ";
    }
}

// CountVisitor 实现
CountVisitor::CountVisitor() : nodeCount_(0) {}

void CountVisitor::visitExpression(Expression* expr) {
    nodeCount_++;
    BaseVisitor::visitExpression(expr);
}

void CountVisitor::visitStatement(Statement* stmt) {
    nodeCount_++;
    BaseVisitor::visitStatement(stmt);
}

void CountVisitor::visitDeclaration(Declaration* decl) {
    nodeCount_++;
    BaseVisitor::visitDeclaration(decl);
}

void CountVisitor::visitType(Type* type) {
    nodeCount_++;
    BaseVisitor::visitType(type);
}

int CountVisitor::getNodeCount() const {
    return nodeCount_;
}

// ValidateVisitor 实现
ValidateVisitor::ValidateVisitor() : hasErrors_(false) {}

void ValidateVisitor::visitExpression(Expression* expr) {
    if (!expr) {
        errors_.push_back("空表达式节点");
        hasErrors_ = true;
        return;
    }
    
    // 验证表达式的基本属性
    if (expr->getTypeName().empty()) {
        errors_.push_back("表达式缺少类型名称");
        hasErrors_ = true;
    }
    
    BaseVisitor::visitExpression(expr);
}

void ValidateVisitor::visitStatement(Statement* stmt) {
    if (!stmt) {
        errors_.push_back("空语句节点");
        hasErrors_ = true;
        return;
    }
    
    BaseVisitor::visitStatement(stmt);
}

void ValidateVisitor::visitDeclaration(Declaration* decl) {
    if (!decl) {
        errors_.push_back("空声明节点");
        hasErrors_ = true;
        return;
    }
    
    BaseVisitor::visitDeclaration(decl);
}

void ValidateVisitor::visitType(Type* type) {
    if (!type) {
        errors_.push_back("空类型节点");
        hasErrors_ = true;
        return;
    }
    
    BaseVisitor::visitType(type);
}

bool ValidateVisitor::hasErrors() const {
    return hasErrors_;
}

const std::vector<std::string>& ValidateVisitor::getErrors() const {
    return errors_;
}

} // namespace ast
} // namespace starry