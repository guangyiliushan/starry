#include "starry/AST.h"
#include <iostream>
#include <memory>

namespace starry {
namespace ast {

// 变量声明节点实现
class VariableDeclaration : public Declaration {
private:
    std::string name;
    std::string type;
    std::unique_ptr<Expression> initializer;
    
public:
    VariableDeclaration(const std::string& name, const std::string& type, 
                       std::unique_ptr<Expression> init = nullptr)
        : name(name), type(type), initializer(std::move(init)) {}
    
    void accept(ASTVisitor& visitor) override {
        visitor.visitVariableDeclaration(*this);
    }
    
    const std::string& getName() const { return name; }
    const std::string& getType() const { return type; }
    Expression* getInitializer() const { return initializer.get(); }
    
    void print(int indent = 0) const override {
        std::string spaces(indent, ' ');
        std::cout << spaces << "变量声明: " << name << " : " << type;
        if (initializer) {
            std::cout << " = ";
            initializer->print(0);
        }
        std::cout << std::endl;
    }
};

// 函数声明节点实现
class FunctionDeclaration : public Declaration {
private:
    std::string name;
    std::string returnType;
    std::vector<std::pair<std::string, std::string>> parameters;
    std::unique_ptr<Statement> body;
    
public:
    FunctionDeclaration(const std::string& name, const std::string& returnType,
                       std::vector<std::pair<std::string, std::string>> params,
                       std::unique_ptr<Statement> body)
        : name(name), returnType(returnType), parameters(std::move(params)), 
          body(std::move(body)) {}
    
    void accept(ASTVisitor& visitor) override {
        visitor.visitFunctionDeclaration(*this);
    }
    
    const std::string& getName() const { return name; }
    const std::string& getReturnType() const { return returnType; }
    const std::vector<std::pair<std::string, std::string>>& getParameters() const { 
        return parameters; 
    }
    Statement* getBody() const { return body.get(); }
    
    void print(int indent = 0) const override {
        std::string spaces(indent, ' ');
        std::cout << spaces << "函数声明: " << name << "(";
        for (size_t i = 0; i < parameters.size(); ++i) {
            if (i > 0) std::cout << ", ";
            std::cout << parameters[i].first << ": " << parameters[i].second;
        }
        std::cout << ") -> " << returnType << std::endl;
        if (body) {
            body->print(indent + 2);
        }
    }
};

// 类声明节点实现
class ClassDeclaration : public Declaration {
private:
    std::string name;
    std::string baseClass;
    std::vector<std::unique_ptr<Declaration>> members;
    
public:
    ClassDeclaration(const std::string& name, const std::string& baseClass = "")
        : name(name), baseClass(baseClass) {}
    
    void accept(ASTVisitor& visitor) override {
        visitor.visitClassDeclaration(*this);
    }
    
    void addMember(std::unique_ptr<Declaration> member) {
        members.push_back(std::move(member));
    }
    
    const std::string& getName() const { return name; }
    const std::string& getBaseClass() const { return baseClass; }
    const std::vector<std::unique_ptr<Declaration>>& getMembers() const { 
        return members; 
    }
    
    void print(int indent = 0) const override {
        std::string spaces(indent, ' ');
        std::cout << spaces << "类声明: " << name;
        if (!baseClass.empty()) {
            std::cout << " extends " << baseClass;
        }
        std::cout << " {" << std::endl;
        for (const auto& member : members) {
            member->print(indent + 2);
        }
        std::cout << spaces << "}" << std::endl;
    }
};

// 接口声明节点实现
class InterfaceDeclaration : public Declaration {
private:
    std::string name;
    std::vector<std::unique_ptr<Declaration>> methods;
    
public:
    InterfaceDeclaration(const std::string& name) : name(name) {}
    
    void accept(ASTVisitor& visitor) override {
        visitor.visitInterfaceDeclaration(*this);
    }
    
    void addMethod(std::unique_ptr<Declaration> method) {
        methods.push_back(std::move(method));
    }
    
    const std::string& getName() const { return name; }
    const std::vector<std::unique_ptr<Declaration>>& getMethods() const { 
        return methods; 
    }
    
    void print(int indent = 0) const override {
        std::string spaces(indent, ' ');
        std::cout << spaces << "接口声明: " << name << " {" << std::endl;
        for (const auto& method : methods) {
            method->print(indent + 2);
        }
        std::cout << spaces << "}" << std::endl;
    }
};

// 命名空间声明节点实现
class NamespaceDeclaration : public Declaration {
private:
    std::string name;
    std::vector<std::unique_ptr<Declaration>> declarations;
    
public:
    NamespaceDeclaration(const std::string& name) : name(name) {}
    
    void accept(ASTVisitor& visitor) override {
        visitor.visitNamespaceDeclaration(*this);
    }
    
    void addDeclaration(std::unique_ptr<Declaration> decl) {
        declarations.push_back(std::move(decl));
    }
    
    const std::string& getName() const { return name; }
    const std::vector<std::unique_ptr<Declaration>>& getDeclarations() const { 
        return declarations; 
    }
    
    void print(int indent = 0) const override {
        std::string spaces(indent, ' ');
        std::cout << spaces << "命名空间: " << name << " {" << std::endl;
        for (const auto& decl : declarations) {
            decl->print(indent + 2);
        }
        std::cout << spaces << "}" << std::endl;
    }
};

// 导入声明节点实现
class ImportDeclaration : public Declaration {
private:
    std::string moduleName;
    std::vector<std::string> importedNames;
    bool importAll;
    
public:
    ImportDeclaration(const std::string& moduleName, bool importAll = false)
        : moduleName(moduleName), importAll(importAll) {}
    
    ImportDeclaration(const std::string& moduleName, 
                     const std::vector<std::string>& names)
        : moduleName(moduleName), importedNames(names), importAll(false) {}
    
    void accept(ASTVisitor& visitor) override {
        visitor.visitImportDeclaration(*this);
    }
    
    const std::string& getModuleName() const { return moduleName; }
    const std::vector<std::string>& getImportedNames() const { return importedNames; }
    bool isImportAll() const { return importAll; }
    
    void print(int indent = 0) const override {
        std::string spaces(indent, ' ');
        std::cout << spaces << "导入声明: ";
        if (importAll) {
            std::cout << "import * from " << moduleName;
        } else if (importedNames.empty()) {
            std::cout << "import " << moduleName;
        } else {
            std::cout << "import {";
            for (size_t i = 0; i < importedNames.size(); ++i) {
                if (i > 0) std::cout << ", ";
                std::cout << importedNames[i];
            }
            std::cout << "} from " << moduleName;
        }
        std::cout << std::endl;
    }
};

} // namespace ast
} // namespace starry