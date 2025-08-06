#include "starry/semantic/SymbolTable.h"
#include <iostream>
#include <unordered_map>
#include <memory>

namespace starry {
namespace semantic {

// 作用域实现
class Scope {
private:
    std::unordered_map<std::string, std::string> variables;
    std::unordered_map<std::string, std::string> functions;
    std::unordered_map<std::string, std::string> types;
    std::unique_ptr<Scope> parent;
    std::vector<std::unique_ptr<Scope>> children;
    std::string scopeName;
    int scopeLevel;
    
public:
    Scope(const std::string& name = "global", Scope* parentScope = nullptr, int level = 0)
        : scopeName(name), scopeLevel(level) {
        if (parentScope) {
            parent = std::unique_ptr<Scope>(parentScope);
        }
    }
    
    ~Scope() = default;
    
    // 添加变量
    bool addVariable(const std::string& name, const std::string& type) {
        if (variables.find(name) != variables.end()) {
            return false; // 变量已存在
        }
        variables[name] = type;
        return true;
    }
    
    // 查找变量（递归查找父作用域）
    std::string findVariable(const std::string& name) const {
        auto it = variables.find(name);
        if (it != variables.end()) {
            return it->second;
        }
        
        if (parent) {
            return parent->findVariable(name);
        }
        
        return ""; // 未找到
    }
    
    // 检查变量是否存在
    bool hasVariable(const std::string& name) const {
        return !findVariable(name).empty();
    }
    
    // 添加函数
    bool addFunction(const std::string& name, const std::string& signature) {
        if (functions.find(name) != functions.end()) {
            return false; // 函数已存在
        }
        functions[name] = signature;
        return true;
    }
    
    // 查找函数
    std::string findFunction(const std::string& name) const {
        auto it = functions.find(name);
        if (it != functions.end()) {
            return it->second;
        }
        
        if (parent) {
            return parent->findFunction(name);
        }
        
        return ""; // 未找到
    }
    
    // 检查函数是否存在
    bool hasFunction(const std::string& name) const {
        return !findFunction(name).empty();
    }
    
    // 添加类型
    bool addType(const std::string& name, const std::string& kind) {
        if (types.find(name) != types.end()) {
            return false; // 类型已存在
        }
        types[name] = kind;
        return true;
    }
    
    // 查找类型
    std::string findType(const std::string& name) const {
        auto it = types.find(name);
        if (it != types.end()) {
            return it->second;
        }
        
        if (parent) {
            return parent->findType(name);
        }
        
        return ""; // 未找到
    }
    
    // 检查类型是否存在
    bool hasType(const std::string& name) const {
        return !findType(name).empty();
    }
    
    // 创建子作用域
    Scope* createChildScope(const std::string& name) {
        auto child = std::make_unique<Scope>(name, this, scopeLevel + 1);
        Scope* childPtr = child.get();
        children.push_back(std::move(child));
        return childPtr;
    }
    
    // 获取父作用域
    Scope* getParent() const {
        return parent.get();
    }
    
    // 获取作用域名称
    const std::string& getName() const {
        return scopeName;
    }
    
    // 获取作用域层级
    int getLevel() const {
        return scopeLevel;
    }
    
    // 获取所有变量
    const std::unordered_map<std::string, std::string>& getVariables() const {
        return variables;
    }
    
    // 获取所有函数
    const std::unordered_map<std::string, std::string>& getFunctions() const {
        return functions;
    }
    
    // 获取所有类型
    const std::unordered_map<std::string, std::string>& getTypes() const {
        return types;
    }
    
    // 清空当前作用域
    void clear() {
        variables.clear();
        functions.clear();
        types.clear();
        children.clear();
    }
    
    // 获取作用域大小
    size_t size() const {
        return variables.size() + functions.size() + types.size();
    }
    
    // 检查作用域是否为空
    bool empty() const {
        return variables.empty() && functions.empty() && types.empty();
    }
    
    // 打印作用域信息
    void print(int indent = 0) const {
        std::string spaces(indent, ' ');
        std::cout << spaces << "作用域: " << scopeName << " (层级: " << scopeLevel << ")" << std::endl;
        
        if (!variables.empty()) {
            std::cout << spaces << "  变量:" << std::endl;
            for (const auto& var : variables) {
                std::cout << spaces << "    " << var.first << " : " << var.second << std::endl;
            }
        }
        
        if (!functions.empty()) {
            std::cout << spaces << "  函数:" << std::endl;
            for (const auto& func : functions) {
                std::cout << spaces << "    " << func.first << " : " << func.second << std::endl;
            }
        }
        
        if (!types.empty()) {
            std::cout << spaces << "  类型:" << std::endl;
            for (const auto& type : types) {
                std::cout << spaces << "    " << type.first << " : " << type.second << std::endl;
            }
        }
        
        for (const auto& child : children) {
            child->print(indent + 2);
        }
    }
    
    // 删除变量
    bool removeVariable(const std::string& name) {
        return variables.erase(name) > 0;
    }
    
    // 删除函数
    bool removeFunction(const std::string& name) {
        return functions.erase(name) > 0;
    }
    
    // 删除类型
    bool removeType(const std::string& name) {
        return types.erase(name) > 0;
    }
    
    // 获取作用域路径
    std::string getPath() const {
        if (parent) {
            return parent->getPath() + "::" + scopeName;
        }
        return scopeName;
    }
    
    // 查找最近的包含指定变量的作用域
    Scope* findScopeWithVariable(const std::string& name) {
        if (variables.find(name) != variables.end()) {
            return this;
        }
        
        if (parent) {
            return parent->findScopeWithVariable(name);
        }
        
        return nullptr;
    }
    
    // 查找最近的包含指定函数的作用域
    Scope* findScopeWithFunction(const std::string& name) {
        if (functions.find(name) != functions.end()) {
            return this;
        }
        
        if (parent) {
            return parent->findScopeWithFunction(name);
        }
        
        return nullptr;
    }
    
    // 查找最近的包含指定类型的作用域
    Scope* findScopeWithType(const std::string& name) {
        if (types.find(name) != types.end()) {
            return this;
        }
        
        if (parent) {
            return parent->findScopeWithType(name);
        }
        
        return nullptr;
    }
    
    // 获取所有子作用域
    const std::vector<std::unique_ptr<Scope>>& getChildren() const {
        return children;
    }
    
    // 统计总符号数量（包括子作用域）
    size_t getTotalSymbolCount() const {
        size_t count = size();
        for (const auto& child : children) {
            count += child->getTotalSymbolCount();
        }
        return count;
    }
    
    // 检查是否为全局作用域
    bool isGlobal() const {
        return parent == nullptr;
    }
    
    // 检查是否为叶子作用域（没有子作用域）
    bool isLeaf() const {
        return children.empty();
    }
};

// 作用域管理器实现
class ScopeManager {
private:
    std::unique_ptr<Scope> globalScope;
    Scope* currentScope;
    
public:
    ScopeManager() {
        globalScope = std::make_unique<Scope>("global");
        currentScope = globalScope.get();
    }
    
    ~ScopeManager() = default;
    
    // 进入新作用域
    void enterScope(const std::string& name) {
        currentScope = currentScope->createChildScope(name);
    }
    
    // 退出当前作用域
    void exitScope() {
        if (currentScope && currentScope->getParent()) {
            currentScope = currentScope->getParent();
        }
    }
    
    // 获取当前作用域
    Scope* getCurrentScope() const {
        return currentScope;
    }
    
    // 获取全局作用域
    Scope* getGlobalScope() const {
        return globalScope.get();
    }
    
    // 添加变量到当前作用域
    bool addVariable(const std::string& name, const std::string& type) {
        return currentScope->addVariable(name, type);
    }
    
    // 查找变量
    std::string findVariable(const std::string& name) const {
        return currentScope->findVariable(name);
    }
    
    // 添加函数到当前作用域
    bool addFunction(const std::string& name, const std::string& signature) {
        return currentScope->addFunction(name, signature);
    }
    
    // 查找函数
    std::string findFunction(const std::string& name) const {
        return currentScope->findFunction(name);
    }
    
    // 添加类型到当前作用域
    bool addType(const std::string& name, const std::string& kind) {
        return currentScope->addType(name, kind);
    }
    
    // 查找类型
    std::string findType(const std::string& name) const {
        return currentScope->findType(name);
    }
    
    // 打印所有作用域
    void printAll() const {
        globalScope->print();
    }
    
    // 重置到全局作用域
    void reset() {
        currentScope = globalScope.get();
    }
    
    // 获取当前作用域路径
    std::string getCurrentPath() const {
        return currentScope->getPath();
    }
    
    // 获取当前作用域层级
    int getCurrentLevel() const {
        return currentScope->getLevel();
    }
};

} // namespace semantic
} // namespace starry