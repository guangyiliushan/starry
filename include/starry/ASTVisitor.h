#ifndef STARRY_AST_VISITOR_H
#define STARRY_AST_VISITOR_H

namespace starry {

// 前向声明
class ProgramNode;
class BinaryExpressionNode;
class UnaryExpressionNode;
class LiteralExpressionNode;
class IdentifierExpressionNode;
class AssignmentExpressionNode;
class CallExpressionNode;
class MemberAccessExpressionNode;
class ParameterNode;
class ExpressionStatementNode;
class VariableDeclarationNode;
class FunctionDeclarationNode;
class ClassDeclarationNode;
class BlockStatementNode;
class IfStatementNode;
class WhileStatementNode;
class ForStatementNode;
class ReturnStatementNode;
class BreakStatementNode;
class ContinueStatementNode;

/**
 * AST访问者基类
 * 使用访问者模式遍历和处理AST节点
 */
class ASTVisitor {
public:
    virtual ~ASTVisitor() = default;

    // 表达式节点访问方法
    virtual void visitBinaryExpressionNode(BinaryExpressionNode& node) = 0;
    virtual void visitUnaryExpressionNode(UnaryExpressionNode& node) = 0;
    virtual void visitLiteralExpressionNode(LiteralExpressionNode& node) = 0;
    virtual void visitIdentifierExpressionNode(IdentifierExpressionNode& node) = 0;
    virtual void visitAssignmentExpressionNode(AssignmentExpressionNode& node) = 0;
    virtual void visitCallExpressionNode(CallExpressionNode& node) = 0;
    virtual void visitMemberAccessExpressionNode(MemberAccessExpressionNode& node) = 0;

    // 语句节点访问方法
    virtual void visitExpressionStatementNode(ExpressionStatementNode& node) = 0;
    virtual void visitVariableDeclarationNode(VariableDeclarationNode& node) = 0;
    virtual void visitFunctionDeclarationNode(FunctionDeclarationNode& node) = 0;
    virtual void visitClassDeclarationNode(ClassDeclarationNode& node) = 0;
    virtual void visitBlockStatementNode(BlockStatementNode& node) = 0;
    virtual void visitIfStatementNode(IfStatementNode& node) = 0;
    virtual void visitWhileStatementNode(WhileStatementNode& node) = 0;
    virtual void visitForStatementNode(ForStatementNode& node) = 0;
    virtual void visitReturnStatementNode(ReturnStatementNode& node) = 0;
    virtual void visitBreakStatementNode(BreakStatementNode& node) = 0;
    virtual void visitContinueStatementNode(ContinueStatementNode& node) = 0;

    // 其他节点访问方法
    virtual void visitProgramNode(ProgramNode& node) = 0;
    virtual void visitParameterNode(ParameterNode& node) = 0;
};

/**
 * 基础AST访问者实现
 * 提供默认的遍历行为
 */
class BaseASTVisitor : public ASTVisitor {
public:
    void visitProgramNode(ProgramNode& node) override;
    void visitBinaryExpressionNode(BinaryExpressionNode& node) override;
    void visitUnaryExpressionNode(UnaryExpressionNode& node) override;
    void visitLiteralExpressionNode(LiteralExpressionNode& node) override;
    void visitIdentifierExpressionNode(IdentifierExpressionNode& node) override;
    void visitAssignmentExpressionNode(AssignmentExpressionNode& node) override;
    void visitCallExpressionNode(CallExpressionNode& node) override;
    void visitMemberAccessExpressionNode(MemberAccessExpressionNode& node) override;
    void visitParameterNode(ParameterNode& node) override;
    void visitExpressionStatementNode(ExpressionStatementNode& node) override;
    void visitVariableDeclarationNode(VariableDeclarationNode& node) override;
    void visitFunctionDeclarationNode(FunctionDeclarationNode& node) override;
    void visitClassDeclarationNode(ClassDeclarationNode& node) override;
    void visitBlockStatementNode(BlockStatementNode& node) override;
    void visitIfStatementNode(IfStatementNode& node) override;
    void visitWhileStatementNode(WhileStatementNode& node) override;
    void visitForStatementNode(ForStatementNode& node) override;
    void visitReturnStatementNode(ReturnStatementNode& node) override;
    void visitBreakStatementNode(BreakStatementNode& node) override;
    void visitContinueStatementNode(ContinueStatementNode& node) override;
};

/**
 * AST打印访问者
 * 用于调试和可视化AST结构
 */
class ASTPrintVisitor : public BaseASTVisitor {
public:
    explicit ASTPrintVisitor(int indent = 0) : indent_level_(indent) {}

    void visitProgramNode(ProgramNode& node) override;
    void visitBinaryExpressionNode(BinaryExpressionNode& node) override;
    void visitUnaryExpressionNode(UnaryExpressionNode& node) override;
    void visitLiteralExpressionNode(LiteralExpressionNode& node) override;
    void visitIdentifierExpressionNode(IdentifierExpressionNode& node) override;
    void visitAssignmentExpressionNode(AssignmentExpressionNode& node) override;
    void visitCallExpressionNode(CallExpressionNode& node) override;
    void visitMemberAccessExpressionNode(MemberAccessExpressionNode& node) override;
    void visitParameterNode(ParameterNode& node) override;
    void visitExpressionStatementNode(ExpressionStatementNode& node) override;
    void visitVariableDeclarationNode(VariableDeclarationNode& node) override;
    void visitFunctionDeclarationNode(FunctionDeclarationNode& node) override;
    void visitClassDeclarationNode(ClassDeclarationNode& node) override;
    void visitBlockStatementNode(BlockStatementNode& node) override;
    void visitIfStatementNode(IfStatementNode& node) override;
    void visitWhileStatementNode(WhileStatementNode& node) override;
    void visitForStatementNode(ForStatementNode& node) override;
    void visitReturnStatementNode(ReturnStatementNode& node) override;
    void visitBreakStatementNode(BreakStatementNode& node) override;
    void visitContinueStatementNode(ContinueStatementNode& node) override;

    std::string getResult() const { return result_; }

private:
    int indent_level_;
    std::string result_;
    
    void printIndent();
    void increaseIndent() { indent_level_++; }
    void decreaseIndent() { indent_level_--; }
};

} // namespace starry

#endif // STARRY_AST_VISITOR_H