#include "starry/semantic/TypeChecker.h"
#include "starry/AST.h"
#include <iostream>
#include <unordered_map>
#include <vector>
#include <memory>

namespace starry {
namespace semantic {

// 语义分析器实现
class SemanticAnalyzer {
private:
    std::unique_ptr<TypeChecker> typeChecker;
    std::vector<std::string> errors;
    std::vector<std::string> warnings;
    
    // 作用域管理
    struct Scope {
        std::unordered_map<std::string, std::string> variables;
        std::unordered_map<std::string, std::string> functions;
        std::unordered_map<std::string, std::string> types;
        Scope* parent;
        
        Scope(Scope* parent = nullptr) : parent(parent) {}
    };
    
    std::unique_ptr<Scope> currentScope;
    
public:
    SemanticAnalyzer() : typeChecker(std::make_unique<TypeChecker>()) {
        currentScope = std::make_unique<Scope>();
        initializeBuiltinTypes();
    }
    
    // 初始化内置类型
    void initializeBuiltinTypes() {
        currentScope->types["int"] = "int";
        currentScope->types["float"] = "float";
        currentScope->types["string"] = "string";
        currentScope->types["bool"] = "bool";
        currentScope->types["void"] = "void";
        currentScope->types["auto"] = "auto";
    }
    
    // 进入新作用域
    void enterScope() {
        auto newScope = std::make_unique<Scope>(currentScope.release());
        currentScope = std::move(newScope);
    }
    
    // 退出当前作用域
    void exitScope() {
        if (currentScope->parent) {
            auto parent = std::unique_ptr<Scope>(currentScope->parent);
            currentScope = std::move(parent);
        }
    }
    
    // 查找变量
    std::string findVariable(const std::string& name) {
        Scope* scope = currentScope.get();
        while (scope) {
            auto it = scope->variables.find(name);
            if (it != scope->variables.end()) {
                return it->second;
            }
            scope = scope->parent;
        }
        return "";
    }
    
    // 声明变量
    bool declareVariable(const std::string& name, const std::string& type) {
        if (currentScope->variables.find(name) != currentScope->variables.end()) {
            addError("变量 '" + name + "' 已经在当前作用域中声明");
            return false;
        }
        currentScope->variables[name] = type;
        return true;
    }
    
    // 查找函数
    std::string findFunction(const std::string& name) {
        Scope* scope = currentScope.get();
        while (scope) {
            auto it = scope->functions.find(name);
            if (it != scope->functions.end()) {
                return it->second;
            }
            scope = scope->parent;
        }
        return "";
    }
    
    // 声明函数
    bool declareFunction(const std::string& name, const std::string& signature) {
        if (currentScope->functions.find(name) != currentScope->functions.end()) {
            addError("函数 '" + name + "' 已经在当前作用域中声明");
            return false;
        }
        currentScope->functions[name] = signature;
        return true;
    }
    
    // 添加错误
    void addError(const std::string& error) {
        errors.push_back(error);
    }
    
    // 添加警告
    void addWarning(const std::string& warning) {
        warnings.push_back(warning);
    }
    
    // 获取错误列表
    const std::vector<std::string>& getErrors() const {
        return errors;
    }
    
    // 获取警告列表
    const std::vector<std::string>& getWarnings() const {
        return warnings;
    }
    
    // 检查是否有错误
    bool hasErrors() const {
        return !errors.empty();
    }
    
    // 分析AST节点
    void analyze(ast::ASTNode* node) {
        if (!node) return;
        
        // 使用访问者模式进行语义分析
        node->accept(*this);
    }
    
    // 访问者模式实现
    void visitProgram(ast::Program& program) {
        for (auto& stmt : program.getStatements()) {
            analyze(stmt.get());
        }
    }
    
    void visitVariableDeclaration(ast::VariableDeclaration& decl) {
        std::string type = decl.getType();
        
        // 检查类型是否存在
        if (currentScope->types.find(type) == currentScope->types.end()) {
            addError("未知类型: " + type);
            return;
        }
        
        // 声明变量
        if (!declareVariable(decl.getName(), type)) {
            return;
        }
        
        // 检查初始化表达式
        if (decl.getInitializer()) {
            analyze(decl.getInitializer());
            
            // 类型检查
            std::string initType = typeChecker->getExpressionType(decl.getInitializer());
            if (!typeChecker->isCompatible(type, initType)) {
                addError("类型不匹配: 无法将 '" + initType + "' 赋值给 '" + type + "'");
            }
        }
    }
    
    void visitFunctionDeclaration(ast::FunctionDeclaration& decl) {
        // 构建函数签名
        std::string signature = decl.getReturnType() + "(";
        const auto& params = decl.getParameters();
        for (size_t i = 0; i < params.size(); ++i) {
            if (i > 0) signature += ",";
            signature += params[i].second;
        }
        signature += ")";
        
        // 声明函数
        if (!declareFunction(decl.getName(), signature)) {
            return;
        }
        
        // 进入函数作用域
        enterScope();
        
        // 声明参数
        for (const auto& param : params) {
            declareVariable(param.first, param.second);
        }
        
        // 分析函数体
        if (decl.getBody()) {
            analyze(decl.getBody());
        }
        
        // 退出函数作用域
        exitScope();
    }
    
    void visitBinaryExpression(ast::BinaryExpression& expr) {
        analyze(expr.getLeft());
        analyze(expr.getRight());
        
        // 类型检查
        std::string leftType = typeChecker->getExpressionType(expr.getLeft());
        std::string rightType = typeChecker->getExpressionType(expr.getRight());
        
        if (!typeChecker->isValidBinaryOperation(expr.getOperator(), leftType, rightType)) {
            addError("无效的二元操作: " + leftType + " " + expr.getOperator() + " " + rightType);
        }
    }
    
    void visitIdentifier(ast::Identifier& id) {
        std::string type = findVariable(id.getName());
        if (type.empty()) {
            addError("未声明的变量: " + id.getName());
        }
    }
    
    // 打印分析结果
    void printResults() {
        if (!errors.empty()) {
            std::cout << "=== 语义错误 ===" << std::endl;
            for (const auto& error : errors) {
                std::cout << "错误: " << error << std::endl;
            }
        }
        
        if (!warnings.empty()) {
            std::cout << "=== 警告 ===" << std::endl;
            for (const auto& warning : warnings) {
                std::cout << "警告: " << warning << std::endl;
            }
        }
        
        if (errors.empty() && warnings.empty()) {
            std::cout << "语义分析通过，没有发现错误或警告" << std::endl;
        }
    }
};

// 全局语义分析函数
bool performSemanticAnalysis(ast::ASTNode* root) {
    SemanticAnalyzer analyzer;
    analyzer.analyze(root);
    analyzer.printResults();
    return !analyzer.hasErrors();
}

} // namespace semantic
} // namespace starry