#include "starry/AST.h"
#include <iostream>
#include <memory>
#include <vector>

namespace starry {
namespace ast {

// 基础类型实现
class PrimitiveType : public Type {
private:
    std::string typeName;
    
public:
    PrimitiveType(const std::string& name) : typeName(name) {}
    
    void accept(ASTVisitor& visitor) override {
        visitor.visitPrimitiveType(*this);
    }
    
    const std::string& getName() const { return typeName; }
    
    void print(int indent = 0) const override {
        std::string spaces(indent, ' ');
        std::cout << spaces << "基础类型: " << typeName << std::endl;
    }
    
    bool equals(const Type& other) const override {
        if (const PrimitiveType* otherPrimitive = dynamic_cast<const PrimitiveType*>(&other)) {
            return typeName == otherPrimitive->typeName;
        }
        return false;
    }
    
    std::string toString() const override {
        return typeName;
    }
};

// 数组类型实现
class ArrayType : public Type {
private:
    std::unique_ptr<Type> elementType;
    int size; // -1表示动态数组
    
public:
    ArrayType(std::unique_ptr<Type> elemType, int arraySize = -1)
        : elementType(std::move(elemType)), size(arraySize) {}
    
    void accept(ASTVisitor& visitor) override {
        visitor.visitArrayType(*this);
    }
    
    Type* getElementType() const { return elementType.get(); }
    int getSize() const { return size; }
    bool isDynamic() const { return size == -1; }
    
    void print(int indent = 0) const override {
        std::string spaces(indent, ' ');
        std::cout << spaces << "数组类型: ";
        elementType->print(0);
        if (size >= 0) {
            std::cout << "[" << size << "]";
        } else {
            std::cout << "[]";
        }
        std::cout << std::endl;
    }
    
    bool equals(const Type& other) const override {
        if (const ArrayType* otherArray = dynamic_cast<const ArrayType*>(&other)) {
            return size == otherArray->size && elementType->equals(*otherArray->elementType);
        }
        return false;
    }
    
    std::string toString() const override {
        std::string result = elementType->toString();
        if (size >= 0) {
            result += "[" + std::to_string(size) + "]";
        } else {
            result += "[]";
        }
        return result;
    }
};

// 函数类型实现
class FunctionType : public Type {
private:
    std::unique_ptr<Type> returnType;
    std::vector<std::unique_ptr<Type>> parameterTypes;
    
public:
    FunctionType(std::unique_ptr<Type> retType, std::vector<std::unique_ptr<Type>> paramTypes)
        : returnType(std::move(retType)), parameterTypes(std::move(paramTypes)) {}
    
    void accept(ASTVisitor& visitor) override {
        visitor.visitFunctionType(*this);
    }
    
    Type* getReturnType() const { return returnType.get(); }
    const std::vector<std::unique_ptr<Type>>& getParameterTypes() const { 
        return parameterTypes; 
    }
    
    void print(int indent = 0) const override {
        std::string spaces(indent, ' ');
        std::cout << spaces << "函数类型: (";
        for (size_t i = 0; i < parameterTypes.size(); ++i) {
            if (i > 0) std::cout << ", ";
            parameterTypes[i]->print(0);
        }
        std::cout << ") -> ";
        returnType->print(0);
        std::cout << std::endl;
    }
    
    bool equals(const Type& other) const override {
        if (const FunctionType* otherFunc = dynamic_cast<const FunctionType*>(&other)) {
            if (!returnType->equals(*otherFunc->returnType)) return false;
            if (parameterTypes.size() != otherFunc->parameterTypes.size()) return false;
            for (size_t i = 0; i < parameterTypes.size(); ++i) {
                if (!parameterTypes[i]->equals(*otherFunc->parameterTypes[i])) {
                    return false;
                }
            }
            return true;
        }
        return false;
    }
    
    std::string toString() const override {
        std::string result = "(";
        for (size_t i = 0; i < parameterTypes.size(); ++i) {
            if (i > 0) result += ", ";
            result += parameterTypes[i]->toString();
        }
        result += ") -> " + returnType->toString();
        return result;
    }
};

// 指针类型实现
class PointerType : public Type {
private:
    std::unique_ptr<Type> pointeeType;
    
public:
    PointerType(std::unique_ptr<Type> pointee) : pointeeType(std::move(pointee)) {}
    
    void accept(ASTVisitor& visitor) override {
        visitor.visitPointerType(*this);
    }
    
    Type* getPointeeType() const { return pointeeType.get(); }
    
    void print(int indent = 0) const override {
        std::string spaces(indent, ' ');
        std::cout << spaces << "指针类型: *";
        pointeeType->print(0);
        std::cout << std::endl;
    }
    
    bool equals(const Type& other) const override {
        if (const PointerType* otherPtr = dynamic_cast<const PointerType*>(&other)) {
            return pointeeType->equals(*otherPtr->pointeeType);
        }
        return false;
    }
    
    std::string toString() const override {
        return "*" + pointeeType->toString();
    }
};

// 结构体类型实现
class StructType : public Type {
private:
    std::string name;
    std::vector<std::pair<std::string, std::unique_ptr<Type>>> fields;
    
public:
    StructType(const std::string& structName) : name(structName) {}
    
    void accept(ASTVisitor& visitor) override {
        visitor.visitStructType(*this);
    }
    
    void addField(const std::string& fieldName, std::unique_ptr<Type> fieldType) {
        fields.emplace_back(fieldName, std::move(fieldType));
    }
    
    const std::string& getName() const { return name; }
    const std::vector<std::pair<std::string, std::unique_ptr<Type>>>& getFields() const { 
        return fields; 
    }
    
    Type* getFieldType(const std::string& fieldName) const {
        for (const auto& field : fields) {
            if (field.first == fieldName) {
                return field.second.get();
            }
        }
        return nullptr;
    }
    
    void print(int indent = 0) const override {
        std::string spaces(indent, ' ');
        std::cout << spaces << "结构体类型: " << name << " {" << std::endl;
        for (const auto& field : fields) {
            std::cout << spaces << "  " << field.first << ": ";
            field.second->print(0);
        }
        std::cout << spaces << "}" << std::endl;
    }
    
    bool equals(const Type& other) const override {
        if (const StructType* otherStruct = dynamic_cast<const StructType*>(&other)) {
            return name == otherStruct->name;
        }
        return false;
    }
    
    std::string toString() const override {
        return "struct " + name;
    }
};

// 类类型实现
class ClassType : public Type {
private:
    std::string name;
    std::string baseClass;
    std::vector<std::pair<std::string, std::unique_ptr<Type>>> members;
    std::vector<std::string> methods;
    
public:
    ClassType(const std::string& className, const std::string& base = "")
        : name(className), baseClass(base) {}
    
    void accept(ASTVisitor& visitor) override {
        visitor.visitClassType(*this);
    }
    
    void addMember(const std::string& memberName, std::unique_ptr<Type> memberType) {
        members.emplace_back(memberName, std::move(memberType));
    }
    
    void addMethod(const std::string& methodName) {
        methods.push_back(methodName);
    }
    
    const std::string& getName() const { return name; }
    const std::string& getBaseClass() const { return baseClass; }
    const std::vector<std::pair<std::string, std::unique_ptr<Type>>>& getMembers() const { 
        return members; 
    }
    const std::vector<std::string>& getMethods() const { return methods; }
    
    void print(int indent = 0) const override {
        std::string spaces(indent, ' ');
        std::cout << spaces << "类类型: " << name;
        if (!baseClass.empty()) {
            std::cout << " extends " << baseClass;
        }
        std::cout << " {" << std::endl;
        
        for (const auto& member : members) {
            std::cout << spaces << "  " << member.first << ": ";
            member.second->print(0);
        }
        
        for (const auto& method : methods) {
            std::cout << spaces << "  方法: " << method << std::endl;
        }
        
        std::cout << spaces << "}" << std::endl;
    }
    
    bool equals(const Type& other) const override {
        if (const ClassType* otherClass = dynamic_cast<const ClassType*>(&other)) {
            return name == otherClass->name;
        }
        return false;
    }
    
    std::string toString() const override {
        return "class " + name;
    }
};

// 泛型类型实现
class GenericType : public Type {
private:
    std::string name;
    std::vector<std::unique_ptr<Type>> typeParameters;
    
public:
    GenericType(const std::string& typeName) : name(typeName) {}
    
    void accept(ASTVisitor& visitor) override {
        visitor.visitGenericType(*this);
    }
    
    void addTypeParameter(std::unique_ptr<Type> param) {
        typeParameters.push_back(std::move(param));
    }
    
    const std::string& getName() const { return name; }
    const std::vector<std::unique_ptr<Type>>& getTypeParameters() const { 
        return typeParameters; 
    }
    
    void print(int indent = 0) const override {
        std::string spaces(indent, ' ');
        std::cout << spaces << "泛型类型: " << name;
        if (!typeParameters.empty()) {
            std::cout << "<";
            for (size_t i = 0; i < typeParameters.size(); ++i) {
                if (i > 0) std::cout << ", ";
                typeParameters[i]->print(0);
            }
            std::cout << ">";
        }
        std::cout << std::endl;
    }
    
    bool equals(const Type& other) const override {
        if (const GenericType* otherGeneric = dynamic_cast<const GenericType*>(&other)) {
            if (name != otherGeneric->name) return false;
            if (typeParameters.size() != otherGeneric->typeParameters.size()) return false;
            for (size_t i = 0; i < typeParameters.size(); ++i) {
                if (!typeParameters[i]->equals(*otherGeneric->typeParameters[i])) {
                    return false;
                }
            }
            return true;
        }
        return false;
    }
    
    std::string toString() const override {
        std::string result = name;
        if (!typeParameters.empty()) {
            result += "<";
            for (size_t i = 0; i < typeParameters.size(); ++i) {
                if (i > 0) result += ", ";
                result += typeParameters[i]->toString();
            }
            result += ">";
        }
        return result;
    }
};

} // namespace ast
} // namespace starry