/**
 * @file Expression.cpp
 * @brief Starry语言AST表达式节点实现
 * @author Starry Team
 * @date 2024
 */

#include "starry/AST.h"
#include <sstream>

namespace starry {
namespace ast {

// 字面量表达式实现
LiteralExpression::LiteralExpression(const std::string& value, LiteralType type)
    : value_(value), type_(type) {}

std::string LiteralExpression::toString() const {
    return "Literal(" + value_ + ")";
}

void LiteralExpression::accept(ASTVisitor& visitor) {
    visitor.visit(*this);
}

// 标识符表达式实现
IdentifierExpression::IdentifierExpression(const std::string& name)
    : name_(name) {}

std::string IdentifierExpression::toString() const {
    return "Identifier(" + name_ + ")";
}

void IdentifierExpression::accept(ASTVisitor& visitor) {
    visitor.visit(*this);
}

// 二元表达式实现
BinaryExpression::BinaryExpression(std::unique_ptr<Expression> left,
                                   BinaryOperator op,
                                   std::unique_ptr<Expression> right)
    : left_(std::move(left)), operator_(op), right_(std::move(right)) {}

std::string BinaryExpression::toString() const {
    std::string op_str;
    switch (operator_) {
        case BinaryOperator::Add: op_str = "+"; break;
        case BinaryOperator::Subtract: op_str = "-"; break;
        case BinaryOperator::Multiply: op_str = "*"; break;
        case BinaryOperator::Divide: op_str = "/"; break;
        case BinaryOperator::Equal: op_str = "=="; break;
        case BinaryOperator::NotEqual: op_str = "!="; break;
        case BinaryOperator::Less: op_str = "<"; break;
        case BinaryOperator::Greater: op_str = ">"; break;
        case BinaryOperator::LessEqual: op_str = "<="; break;
        case BinaryOperator::GreaterEqual: op_str = ">="; break;
        case BinaryOperator::LogicalAnd: op_str = "&&"; break;
        case BinaryOperator::LogicalOr: op_str = "||"; break;
    }
    
    return "Binary(" + left_->toString() + " " + op_str + " " + right_->toString() + ")";
}

void BinaryExpression::accept(ASTVisitor& visitor) {
    visitor.visit(*this);
}

// 一元表达式实现
UnaryExpression::UnaryExpression(UnaryOperator op, std::unique_ptr<Expression> operand)
    : operator_(op), operand_(std::move(operand)) {}

std::string UnaryExpression::toString() const {
    std::string op_str;
    switch (operator_) {
        case UnaryOperator::Plus: op_str = "+"; break;
        case UnaryOperator::Minus: op_str = "-"; break;
        case UnaryOperator::LogicalNot: op_str = "!"; break;
    }
    
    return "Unary(" + op_str + operand_->toString() + ")";
}

void UnaryExpression::accept(ASTVisitor& visitor) {
    visitor.visit(*this);
}

// 函数调用表达式实现
CallExpression::CallExpression(std::unique_ptr<Expression> callee,
                               std::vector<std::unique_ptr<Expression>> arguments)
    : callee_(std::move(callee)), arguments_(std::move(arguments)) {}

std::string CallExpression::toString() const {
    std::ostringstream oss;
    oss << "Call(" << callee_->toString() << "(";
    for (size_t i = 0; i < arguments_.size(); ++i) {
        if (i > 0) oss << ", ";
        oss << arguments_[i]->toString();
    }
    oss << "))";
    return oss.str();
}

void CallExpression::accept(ASTVisitor& visitor) {
    visitor.visit(*this);
}

} // namespace ast
} // namespace starry