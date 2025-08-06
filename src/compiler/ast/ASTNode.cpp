#include "starry/AST.h"
#include "starry/ASTVisitor.h"

namespace starry {

// ProgramNode实现
void ProgramNode::accept(ASTVisitor& visitor) {
    visitor.visitProgramNode(*this);
}

// BinaryExpressionNode实现
void BinaryExpressionNode::accept(ASTVisitor& visitor) {
    visitor.visitBinaryExpressionNode(*this);
}

// UnaryExpressionNode实现
void UnaryExpressionNode::accept(ASTVisitor& visitor) {
    visitor.visitUnaryExpressionNode(*this);
}

// LiteralExpressionNode实现
void LiteralExpressionNode::accept(ASTVisitor& visitor) {
    visitor.visitLiteralExpressionNode(*this);
}

// IdentifierExpressionNode实现
void IdentifierExpressionNode::accept(ASTVisitor& visitor) {
    visitor.visitIdentifierExpressionNode(*this);
}

// AssignmentExpressionNode实现
void AssignmentExpressionNode::accept(ASTVisitor& visitor) {
    visitor.visitAssignmentExpressionNode(*this);
}

// CallExpressionNode实现
void CallExpressionNode::accept(ASTVisitor& visitor) {
    visitor.visitCallExpressionNode(*this);
}

// MemberAccessExpressionNode实现
void MemberAccessExpressionNode::accept(ASTVisitor& visitor) {
    visitor.visitMemberAccessExpressionNode(*this);
}

// ParameterNode实现
void ParameterNode::accept(ASTVisitor& visitor) {
    visitor.visitParameterNode(*this);
}

// ExpressionStatementNode实现
void ExpressionStatementNode::accept(ASTVisitor& visitor) {
    visitor.visitExpressionStatementNode(*this);
}

// VariableDeclarationNode实现
void VariableDeclarationNode::accept(ASTVisitor& visitor) {
    visitor.visitVariableDeclarationNode(*this);
}

// FunctionDeclarationNode实现
void FunctionDeclarationNode::accept(ASTVisitor& visitor) {
    visitor.visitFunctionDeclarationNode(*this);
}

// ClassDeclarationNode实现
void ClassDeclarationNode::accept(ASTVisitor& visitor) {
    visitor.visitClassDeclarationNode(*this);
}

// BlockStatementNode实现
void BlockStatementNode::accept(ASTVisitor& visitor) {
    visitor.visitBlockStatementNode(*this);
}

// IfStatementNode实现
void IfStatementNode::accept(ASTVisitor& visitor) {
    visitor.visitIfStatementNode(*this);
}

// WhileStatementNode实现
void WhileStatementNode::accept(ASTVisitor& visitor) {
    visitor.visitWhileStatementNode(*this);
}

// ForStatementNode实现
void ForStatementNode::accept(ASTVisitor& visitor) {
    visitor.visitForStatementNode(*this);
}

// ReturnStatementNode实现
void ReturnStatementNode::accept(ASTVisitor& visitor) {
    visitor.visitReturnStatementNode(*this);
}

// BreakStatementNode实现
void BreakStatementNode::accept(ASTVisitor& visitor) {
    visitor.visitBreakStatementNode(*this);
}

// ContinueStatementNode实现
void ContinueStatementNode::accept(ASTVisitor& visitor) {
    visitor.visitContinueStatementNode(*this);
}

} // namespace starry