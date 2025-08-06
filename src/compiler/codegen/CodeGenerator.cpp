/**
 * @file CodeGenerator.cpp
 * @brief Starry语言代码生成器实现
 * @author Starry Team
 * @date 2024
 */

#include "starry/codegen/CodeGenerator.h"
#include <sstream>
#include <stdexcept>

namespace starry {
namespace codegen {

CodeGenerator::CodeGenerator() : output_stream_(&std::cout) {}

CodeGenerator::CodeGenerator(std::ostream& output) : output_stream_(&output) {}

void CodeGenerator::generate(const ast::ASTNode& root) {
    // 开始代码生成
    *output_stream_ << "// Starry语言生成的代码\n";
    *output_stream_ << "#include <iostream>\n";
    *output_stream_ << "#include <string>\n\n";
    
    // 访问AST节点
    root.accept(*this);
}

void CodeGenerator::visit(const ast::LiteralExpression& node) {
    *output_stream_ << node.getValue();
}

void CodeGenerator::visit(const ast::IdentifierExpression& node) {
    *output_stream_ << node.getName();
}

void CodeGenerator::visit(const ast::BinaryExpression& node) {
    *output_stream_ << "(";
    node.getLeft().accept(*this);
    
    switch (node.getOperator()) {
        case ast::BinaryOperator::Add:
            *output_stream_ << " + ";
            break;
        case ast::BinaryOperator::Subtract:
            *output_stream_ << " - ";
            break;
        case ast::BinaryOperator::Multiply:
            *output_stream_ << " * ";
            break;
        case ast::BinaryOperator::Divide:
            *output_stream_ << " / ";
            break;
        case ast::BinaryOperator::Equal:
            *output_stream_ << " == ";
            break;
        case ast::BinaryOperator::NotEqual:
            *output_stream_ << " != ";
            break;
        case ast::BinaryOperator::Less:
            *output_stream_ << " < ";
            break;
        case ast::BinaryOperator::Greater:
            *output_stream_ << " > ";
            break;
        case ast::BinaryOperator::LessEqual:
            *output_stream_ << " <= ";
            break;
        case ast::BinaryOperator::GreaterEqual:
            *output_stream_ << " >= ";
            break;
        case ast::BinaryOperator::LogicalAnd:
            *output_stream_ << " && ";
            break;
        case ast::BinaryOperator::LogicalOr:
            *output_stream_ << " || ";
            break;
    }
    
    node.getRight().accept(*this);
    *output_stream_ << ")";
}

void CodeGenerator::visit(const ast::UnaryExpression& node) {
    switch (node.getOperator()) {
        case ast::UnaryOperator::Plus:
            *output_stream_ << "+";
            break;
        case ast::UnaryOperator::Minus:
            *output_stream_ << "-";
            break;
        case ast::UnaryOperator::LogicalNot:
            *output_stream_ << "!";
            break;
    }
    
    *output_stream_ << "(";
    node.getOperand().accept(*this);
    *output_stream_ << ")";
}

void CodeGenerator::visit(const ast::CallExpression& node) {
    node.getCallee().accept(*this);
    *output_stream_ << "(";
    
    const auto& args = node.getArguments();
    for (size_t i = 0; i < args.size(); ++i) {
        if (i > 0) {
            *output_stream_ << ", ";
        }
        args[i]->accept(*this);
    }
    
    *output_stream_ << ")";
}

void CodeGenerator::generateFunction(const std::string& name, 
                                   const std::vector<std::string>& parameters,
                                   const ast::ASTNode& body) {
    *output_stream_ << "void " << name << "(";
    
    for (size_t i = 0; i < parameters.size(); ++i) {
        if (i > 0) {
            *output_stream_ << ", ";
        }
        *output_stream_ << "auto " << parameters[i];
    }
    
    *output_stream_ << ") {\n";
    body.accept(*this);
    *output_stream_ << "\n}\n\n";
}

void CodeGenerator::generateMain() {
    *output_stream_ << "int main() {\n";
    *output_stream_ << "    // 主函数代码\n";
    *output_stream_ << "    return 0;\n";
    *output_stream_ << "}\n";
}

} // namespace codegen
} // namespace starry