#ifndef STARRY_PARSER_H
#define STARRY_PARSER_H

#include "Lexer.h"
#include "AST.h"
#include <memory>
#include <vector>

namespace starry {

/**
 * 语法分析器类
 * 负责将词法单元序列转换为抽象语法树(AST)
 */
class Parser {
public:
    /**
     * 构造函数
     * @param lexer 词法分析器实例
     */
    explicit Parser(std::unique_ptr<Lexer> lexer);
    
    /**
     * 析构函数
     */
    ~Parser() = default;

    /**
     * 解析程序
     * @return 程序的AST根节点
     */
    std::unique_ptr<ProgramNode> parseProgram();

    /**
     * 获取解析错误信息
     * @return 错误信息列表
     */
    const std::vector<std::string>& getErrors() const { return errors_; }

    /**
     * 检查是否有解析错误
     * @return 如果有错误返回true
     */
    bool hasErrors() const { return !errors_.empty(); }

private:
    std::unique_ptr<Lexer> lexer_;                              // 词法分析器
    std::vector<std::shared_ptr<Token>> tokens_;               // 词法单元列表
    size_t current_index_;                                      // 当前词法单元索引
    Token current_token_;                                       // 当前词法单元
    std::vector<std::string> errors_;                          // 错误信息列表

    // 词法单元操作
    void advance();                         // 前进到下一个词法单元
    bool match(TokenType type);            // 匹配指定类型的词法单元
    bool consume(TokenType type, const std::string& message); // 消费指定类型的词法单元
    void error(const std::string& message); // 记录错误信息

    // 语法分析方法 - 按优先级从低到高排列
    std::unique_ptr<StatementNode> parseStatement();
    std::unique_ptr<StatementNode> parseDeclaration();
    std::unique_ptr<StatementNode> parseVariableDeclaration();
    std::unique_ptr<StatementNode> parseFunctionDeclaration();
    std::unique_ptr<StatementNode> parseClassDeclaration();
    std::unique_ptr<StatementNode> parseIfStatement();
    std::unique_ptr<StatementNode> parseWhileStatement();
    std::unique_ptr<StatementNode> parseForStatement();
    std::unique_ptr<StatementNode> parseReturnStatement();
    std::unique_ptr<StatementNode> parseBreakStatement();
    std::unique_ptr<StatementNode> parseContinueStatement();
    std::unique_ptr<StatementNode> parseBlockStatement();
    std::unique_ptr<StatementNode> parseExpressionStatement();

    // 表达式解析方法
    std::unique_ptr<ExpressionNode> parseExpression();
    std::unique_ptr<ExpressionNode> parseAssignment();
    std::unique_ptr<ExpressionNode> parseLogicalOr();
    std::unique_ptr<ExpressionNode> parseLogicalAnd();
    std::unique_ptr<ExpressionNode> parseEquality();
    std::unique_ptr<ExpressionNode> parseComparison();
    std::unique_ptr<ExpressionNode> parseTerm();
    std::unique_ptr<ExpressionNode> parseFactor();
    std::unique_ptr<ExpressionNode> parseUnary();
    std::unique_ptr<ExpressionNode> parseCall();
    std::unique_ptr<ExpressionNode> parsePrimary();

    // 辅助方法
    std::unique_ptr<ParameterNode> parseParameter();
    std::vector<std::unique_ptr<ParameterNode>> parseParameterList();
    std::vector<std::unique_ptr<ExpressionNode>> parseArgumentList();
    std::string parseType();
    
    // 同步方法 - 用于错误恢复
    void synchronize();
};

} // namespace starry

#endif // STARRY_PARSER_H