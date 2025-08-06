/**
 * @file SymbolTable.cpp
 * @brief Starry语言符号表实现
 * @author Starry Team
 * @date 2024
 */

#include "starry/semantic/SymbolTable.h"
#include <stdexcept>

namespace starry {
namespace semantic {

// 符号实现
Symbol::Symbol(const std::string& name, SymbolType type, const std::string& data_type)
    : name_(name), type_(type), data_type_(data_type), is_initialized_(false) {}

const std::string& Symbol::getName() const {
    return name_;
}

SymbolType Symbol::getType() const {
    return type_;
}

const std::string& Symbol::getDataType() const {
    return data_type_;
}

bool Symbol::isInitialized() const {
    return is_initialized_;
}

void Symbol::setInitialized(bool initialized) {
    is_initialized_ = initialized;
}

// 作用域实现
Scope::Scope(Scope* parent) : parent_(parent) {}

Scope::~Scope() {
    for (auto& child : children_) {
        delete child;
    }
}

void Scope::addSymbol(const std::string& name, std::unique_ptr<Symbol> symbol) {
    if (symbols_.find(name) != symbols_.end()) {
        throw std::runtime_error("符号 '" + name + "' 已经在当前作用域中定义");
    }
    symbols_[name] = std::move(symbol);
}

Symbol* Scope::findSymbol(const std::string& name) {
    auto it = symbols_.find(name);
    if (it != symbols_.end()) {
        return it->second.get();
    }
    
    // 在父作用域中查找
    if (parent_) {
        return parent_->findSymbol(name);
    }
    
    return nullptr;
}

Scope* Scope::createChildScope() {
    Scope* child = new Scope(this);
    children_.push_back(child);
    return child;
}

// 符号表实现
SymbolTable::SymbolTable() {
    global_scope_ = new Scope(nullptr);
    current_scope_ = global_scope_;
}

SymbolTable::~SymbolTable() {
    delete global_scope_;
}

void SymbolTable::enterScope() {
    current_scope_ = current_scope_->createChildScope();
}

void SymbolTable::exitScope() {
    if (current_scope_->parent_) {
        current_scope_ = current_scope_->parent_;
    }
}

void SymbolTable::addSymbol(const std::string& name, SymbolType type, const std::string& data_type) {
    auto symbol = std::make_unique<Symbol>(name, type, data_type);
    current_scope_->addSymbol(name, std::move(symbol));
}

Symbol* SymbolTable::findSymbol(const std::string& name) {
    return current_scope_->findSymbol(name);
}

bool SymbolTable::isSymbolDefined(const std::string& name) {
    return findSymbol(name) != nullptr;
}

} // namespace semantic
} // namespace starry