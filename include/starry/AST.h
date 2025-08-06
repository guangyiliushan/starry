#ifndef STARRY_AST_H
#define STARRY_AST_H

#include "Token.h"
#include <memory>
#include <vector>
#include <string>

#if STARRY_HAS_CONCEPTS
#include <concepts>
#endif

namespace starry {

// 前向声明
class ASTVisitor;

/**
 * AST节点基类
 */
class ASTNode {
public:
    virtual ~ASTNode() = default;
    
    /**
     * 访问者模式接口
     * @param visitor 访问者对象
     */
    virtual void accept(ASTVisitor& visitor) = 0;
    
    /**
     * 获取节点在源代码中的位置
     * @return 行号
     */
    virtual int getLine() const = 0;
    
    /**
     * 获取节点在源代码中的列号
     * @return 列号
     */
    virtual int getColumn() const = 0;
};

/**
 * 表达式节点基类
 */
class ExpressionNode : public ASTNode {
public:
    virtual ~ExpressionNode() = default;
};

/**
 * 语句节点基类
 */
class StatementNode : public ASTNode {
public:
    virtual ~StatementNode() = default;
};

/**
 * 程序节点 - AST根节点
 */
class ProgramNode : public ASTNode {
public:
    std::vector<std::unique_ptr<StatementNode>> statements;
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return 1; }
    int getColumn() const override { return 1; }
};

/**
 * 二元表达式节点
 */
class BinaryExpressionNode : public ExpressionNode {
public:
    std::unique_ptr<ExpressionNode> left;
    Token operator_token;
    std::unique_ptr<ExpressionNode> right;
    
    BinaryExpressionNode(std::unique_ptr<ExpressionNode> left, Token op, std::unique_ptr<ExpressionNode> right)
        : left(std::move(left)), operator_token(op), right(std::move(right)) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return operator_token.getLine(); }
    int getColumn() const override { return operator_token.getColumn(); }
};

/**
 * 一元表达式节点
 */
class UnaryExpressionNode : public ExpressionNode {
public:
    Token operator_token;
    std::unique_ptr<ExpressionNode> operand;
    
    UnaryExpressionNode(Token op, std::unique_ptr<ExpressionNode> operand)
        : operator_token(op), operand(std::move(operand)) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return operator_token.getLine(); }
    int getColumn() const override { return operator_token.getColumn(); }
};

/**
 * 字面量表达式节点
 */
class LiteralExpressionNode : public ExpressionNode {
public:
    Token value;
    
    explicit LiteralExpressionNode(Token value) : value(value) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return value.getLine(); }
    int getColumn() const override { return value.getColumn(); }
};

/**
 * 标识符表达式节点
 */
class IdentifierExpressionNode : public ExpressionNode {
public:
    Token name;
    
    explicit IdentifierExpressionNode(Token name) : name(name) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return name.getLine(); }
    int getColumn() const override { return name.getColumn(); }
};

/**
 * 赋值表达式节点
 */
class AssignmentExpressionNode : public ExpressionNode {
public:
    std::unique_ptr<ExpressionNode> target;
    Token operator_token;
    std::unique_ptr<ExpressionNode> value;
    
    AssignmentExpressionNode(std::unique_ptr<ExpressionNode> target, Token op, std::unique_ptr<ExpressionNode> value)
        : target(std::move(target)), operator_token(op), value(std::move(value)) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return operator_token.getLine(); }
    int getColumn() const override { return operator_token.getColumn(); }
};

/**
 * 函数调用表达式节点
 */
class CallExpressionNode : public ExpressionNode {
public:
    std::unique_ptr<ExpressionNode> callee;
    Token paren;
    std::vector<std::unique_ptr<ExpressionNode>> arguments;
    
    CallExpressionNode(std::unique_ptr<ExpressionNode> callee, Token paren, std::vector<std::unique_ptr<ExpressionNode>> arguments)
        : callee(std::move(callee)), paren(paren), arguments(std::move(arguments)) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return paren.getLine(); }
    int getColumn() const override { return paren.getColumn(); }
};

/**
 * 成员访问表达式节点
 */
class MemberAccessExpressionNode : public ExpressionNode {
public:
    std::unique_ptr<ExpressionNode> object;
    Token dot;
    Token name;
    
    MemberAccessExpressionNode(std::unique_ptr<ExpressionNode> object, Token dot, Token name)
        : object(std::move(object)), dot(dot), name(name) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return dot.getLine(); }
    int getColumn() const override { return dot.getColumn(); }
};

/**
 * 参数节点
 */
class ParameterNode : public ASTNode {
public:
    Token name;
    std::string type;
    
    ParameterNode(Token name, const std::string& type) : name(name), type(type) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return name.getLine(); }
    int getColumn() const override { return name.getColumn(); }
};

/**
 * 表达式语句节点
 */
class ExpressionStatementNode : public StatementNode {
public:
    std::unique_ptr<ExpressionNode> expression;
    
    explicit ExpressionStatementNode(std::unique_ptr<ExpressionNode> expression)
        : expression(std::move(expression)) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return expression->getLine(); }
    int getColumn() const override { return expression->getColumn(); }
};

/**
 * 变量声明语句节点
 */
class VariableDeclarationNode : public StatementNode {
public:
    Token keyword;  // var 或 val
    Token name;
    std::string type;
    std::unique_ptr<ExpressionNode> initializer;
    
    VariableDeclarationNode(Token keyword, Token name, const std::string& type, std::unique_ptr<ExpressionNode> initializer)
        : keyword(keyword), name(name), type(type), initializer(std::move(initializer)) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return keyword.getLine(); }
    int getColumn() const override { return keyword.getColumn(); }
};

/**
 * 函数声明语句节点
 */
class FunctionDeclarationNode : public StatementNode {
public:
    Token name;
    std::vector<std::unique_ptr<ParameterNode>> parameters;
    std::string return_type;
    std::unique_ptr<StatementNode> body;
    
    FunctionDeclarationNode(Token name, std::vector<std::unique_ptr<ParameterNode>> parameters, 
                           const std::string& return_type, std::unique_ptr<StatementNode> body)
        : name(name), parameters(std::move(parameters)), return_type(return_type), body(std::move(body)) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return name.getLine(); }
    int getColumn() const override { return name.getColumn(); }
};

/**
 * 类声明语句节点
 */
class ClassDeclarationNode : public StatementNode {
public:
    Token name;
    Token superclass;
    std::vector<std::unique_ptr<StatementNode>> members;
    
    ClassDeclarationNode(Token name, Token superclass, std::vector<std::unique_ptr<StatementNode>> members)
        : name(name), superclass(superclass), members(std::move(members)) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return name.getLine(); }
    int getColumn() const override { return name.getColumn(); }
};

/**
 * 块语句节点
 */
class BlockStatementNode : public StatementNode {
public:
    std::vector<std::unique_ptr<StatementNode>> statements;
    Token left_brace;
    
    BlockStatementNode(std::vector<std::unique_ptr<StatementNode>> statements, Token left_brace)
        : statements(std::move(statements)), left_brace(left_brace) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return left_brace.getLine(); }
    int getColumn() const override { return left_brace.getColumn(); }
};

/**
 * if语句节点
 */
class IfStatementNode : public StatementNode {
public:
    Token if_token;
    std::unique_ptr<ExpressionNode> condition;
    std::unique_ptr<StatementNode> then_branch;
    std::unique_ptr<StatementNode> else_branch;
    
    IfStatementNode(Token if_token, std::unique_ptr<ExpressionNode> condition, 
                   std::unique_ptr<StatementNode> then_branch, std::unique_ptr<StatementNode> else_branch)
        : if_token(if_token), condition(std::move(condition)), 
          then_branch(std::move(then_branch)), else_branch(std::move(else_branch)) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return if_token.getLine(); }
    int getColumn() const override { return if_token.getColumn(); }
};

/**
 * while语句节点
 */
class WhileStatementNode : public StatementNode {
public:
    Token while_token;
    std::unique_ptr<ExpressionNode> condition;
    std::unique_ptr<StatementNode> body;
    
    WhileStatementNode(Token while_token, std::unique_ptr<ExpressionNode> condition, std::unique_ptr<StatementNode> body)
        : while_token(while_token), condition(std::move(condition)), body(std::move(body)) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return while_token.getLine(); }
    int getColumn() const override { return while_token.getColumn(); }
};

/**
 * for语句节点
 */
class ForStatementNode : public StatementNode {
public:
    Token for_token;
    std::unique_ptr<StatementNode> initializer;
    std::unique_ptr<ExpressionNode> condition;
    std::unique_ptr<ExpressionNode> increment;
    std::unique_ptr<StatementNode> body;
    
    ForStatementNode(Token for_token, std::unique_ptr<StatementNode> initializer,
                    std::unique_ptr<ExpressionNode> condition, std::unique_ptr<ExpressionNode> increment,
                    std::unique_ptr<StatementNode> body)
        : for_token(for_token), initializer(std::move(initializer)), condition(std::move(condition)),
          increment(std::move(increment)), body(std::move(body)) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return for_token.getLine(); }
    int getColumn() const override { return for_token.getColumn(); }
};

/**
 * return语句节点
 */
class ReturnStatementNode : public StatementNode {
public:
    Token return_token;
    std::unique_ptr<ExpressionNode> value;
    
    ReturnStatementNode(Token return_token, std::unique_ptr<ExpressionNode> value)
        : return_token(return_token), value(std::move(value)) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return return_token.getLine(); }
    int getColumn() const override { return return_token.getColumn(); }
};

/**
 * break语句节点
 */
class BreakStatementNode : public StatementNode {
public:
    Token break_token;
    
    explicit BreakStatementNode(Token break_token) : break_token(break_token) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return break_token.getLine(); }
    int getColumn() const override { return break_token.getColumn(); }
};

/**
 * continue语句节点
 */
class ContinueStatementNode : public StatementNode {
public:
    Token continue_token;
    
    explicit ContinueStatementNode(Token continue_token) : continue_token(continue_token) {}
    
    void accept(ASTVisitor& visitor) override;
    int getLine() const override { return continue_token.getLine(); }
    int getColumn() const override { return continue_token.getColumn(); }
};

} // namespace starry

#endif // STARRY_AST_H