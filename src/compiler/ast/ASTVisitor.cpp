#include "starry/ASTVisitor.h"
#include "starry/AST.h"
#include <iostream>
#include <sstream>

namespace starry {

// BaseASTVisitor实现
void BaseASTVisitor::visitProgramNode(ProgramNode& node) {
    for (auto& stmt : node.statements) {
        if (stmt) {
            stmt->accept(*this);
        }
    }
}

void BaseASTVisitor::visitBinaryExpressionNode(BinaryExpressionNode& node) {
    if (node.left) {
        node.left->accept(*this);
    }
    if (node.right) {
        node.right->accept(*this);
    }
}

void BaseASTVisitor::visitUnaryExpressionNode(UnaryExpressionNode& node) {
    if (node.operand) {
        node.operand->accept(*this);
    }
}

void BaseASTVisitor::visitLiteralExpressionNode(LiteralExpressionNode& node) {
    // 字面量节点没有子节点需要访问
}

void BaseASTVisitor::visitIdentifierExpressionNode(IdentifierExpressionNode& node) {
    // 标识符节点没有子节点需要访问
}

void BaseASTVisitor::visitAssignmentExpressionNode(AssignmentExpressionNode& node) {
    if (node.target) {
        node.target->accept(*this);
    }
    if (node.value) {
        node.value->accept(*this);
    }
}

void BaseASTVisitor::visitCallExpressionNode(CallExpressionNode& node) {
    if (node.callee) {
        node.callee->accept(*this);
    }
    for (auto& arg : node.arguments) {
        if (arg) {
            arg->accept(*this);
        }
    }
}

void BaseASTVisitor::visitMemberAccessExpressionNode(MemberAccessExpressionNode& node) {
    if (node.object) {
        node.object->accept(*this);
    }
}

void BaseASTVisitor::visitParameterNode(ParameterNode& node) {
    // 参数节点没有子节点需要访问
}

void BaseASTVisitor::visitExpressionStatementNode(ExpressionStatementNode& node) {
    if (node.expression) {
        node.expression->accept(*this);
    }
}

void BaseASTVisitor::visitVariableDeclarationNode(VariableDeclarationNode& node) {
    if (node.initializer) {
        node.initializer->accept(*this);
    }
}

void BaseASTVisitor::visitFunctionDeclarationNode(FunctionDeclarationNode& node) {
    for (auto& param : node.parameters) {
        if (param) {
            param->accept(*this);
        }
    }
    if (node.body) {
        node.body->accept(*this);
    }
}

void BaseASTVisitor::visitClassDeclarationNode(ClassDeclarationNode& node) {
    for (auto& member : node.members) {
        if (member) {
            member->accept(*this);
        }
    }
}

void BaseASTVisitor::visitBlockStatementNode(BlockStatementNode& node) {
    for (auto& stmt : node.statements) {
        if (stmt) {
            stmt->accept(*this);
        }
    }
}

void BaseASTVisitor::visitIfStatementNode(IfStatementNode& node) {
    if (node.condition) {
        node.condition->accept(*this);
    }
    if (node.then_branch) {
        node.then_branch->accept(*this);
    }
    if (node.else_branch) {
        node.else_branch->accept(*this);
    }
}

void BaseASTVisitor::visitWhileStatementNode(WhileStatementNode& node) {
    if (node.condition) {
        node.condition->accept(*this);
    }
    if (node.body) {
        node.body->accept(*this);
    }
}

void BaseASTVisitor::visitForStatementNode(ForStatementNode& node) {
    if (node.initializer) {
        node.initializer->accept(*this);
    }
    if (node.condition) {
        node.condition->accept(*this);
    }
    if (node.increment) {
        node.increment->accept(*this);
    }
    if (node.body) {
        node.body->accept(*this);
    }
}

void BaseASTVisitor::visitReturnStatementNode(ReturnStatementNode& node) {
    if (node.value) {
        node.value->accept(*this);
    }
}

void BaseASTVisitor::visitBreakStatementNode(BreakStatementNode& node) {
    // break语句没有子节点需要访问
}

void BaseASTVisitor::visitContinueStatementNode(ContinueStatementNode& node) {
    // continue语句没有子节点需要访问
}

// ASTPrintVisitor实现
void ASTPrintVisitor::printIndent() {
    for (int i = 0; i < indent_level_; ++i) {
        result_ += "  ";
    }
}

void ASTPrintVisitor::visitProgramNode(ProgramNode& node) {
    printIndent();
    result_ += "程序节点\n";
    increaseIndent();
    for (auto& stmt : node.statements) {
        if (stmt) {
            stmt->accept(*this);
        }
    }
    decreaseIndent();
}

void ASTPrintVisitor::visitBinaryExpressionNode(BinaryExpressionNode& node) {
    printIndent();
    result_ += "二元表达式: " + node.operator_token.getValue() + "\n";
    increaseIndent();
    if (node.left) {
        node.left->accept(*this);
    }
    if (node.right) {
        node.right->accept(*this);
    }
    decreaseIndent();
}

void ASTPrintVisitor::visitUnaryExpressionNode(UnaryExpressionNode& node) {
    printIndent();
    result_ += "一元表达式: " + node.operator_token.getValue() + "\n";
    increaseIndent();
    if (node.operand) {
        node.operand->accept(*this);
    }
    decreaseIndent();
}

void ASTPrintVisitor::visitLiteralExpressionNode(LiteralExpressionNode& node) {
    printIndent();
    result_ += "字面量: " + node.value.getValue() + "\n";
}

void ASTPrintVisitor::visitIdentifierExpressionNode(IdentifierExpressionNode& node) {
    printIndent();
    result_ += "标识符: " + node.name.getValue() + "\n";
}

void ASTPrintVisitor::visitAssignmentExpressionNode(AssignmentExpressionNode& node) {
    printIndent();
    result_ += "赋值表达式: " + node.operator_token.getValue() + "\n";
    increaseIndent();
    if (node.target) {
        node.target->accept(*this);
    }
    if (node.value) {
        node.value->accept(*this);
    }
    decreaseIndent();
}

void ASTPrintVisitor::visitCallExpressionNode(CallExpressionNode& node) {
    printIndent();
    result_ += "函数调用\n";
    increaseIndent();
    if (node.callee) {
        node.callee->accept(*this);
    }
    printIndent();
    result_ += "参数:\n";
    increaseIndent();
    for (auto& arg : node.arguments) {
        if (arg) {
            arg->accept(*this);
        }
    }
    decreaseIndent();
    decreaseIndent();
}

void ASTPrintVisitor::visitMemberAccessExpressionNode(MemberAccessExpressionNode& node) {
    printIndent();
    result_ += "成员访问: " + node.name.getValue() + "\n";
    increaseIndent();
    if (node.object) {
        node.object->accept(*this);
    }
    decreaseIndent();
}

void ASTPrintVisitor::visitParameterNode(ParameterNode& node) {
    printIndent();
    result_ += "参数: " + node.name.getValue() + " : " + node.type + "\n";
}

void ASTPrintVisitor::visitExpressionStatementNode(ExpressionStatementNode& node) {
    printIndent();
    result_ += "表达式语句\n";
    increaseIndent();
    if (node.expression) {
        node.expression->accept(*this);
    }
    decreaseIndent();
}

void ASTPrintVisitor::visitVariableDeclarationNode(VariableDeclarationNode& node) {
    printIndent();
    result_ += "变量声明: " + node.name.getValue() + " : " + node.type + "\n";
    increaseIndent();
    if (node.initializer) {
        printIndent();
        result_ += "初始化值:\n";
        increaseIndent();
        node.initializer->accept(*this);
        decreaseIndent();
    }
    decreaseIndent();
}

void ASTPrintVisitor::visitFunctionDeclarationNode(FunctionDeclarationNode& node) {
    printIndent();
    result_ += "函数声明: " + node.name.getValue() + " -> " + node.return_type + "\n";
    increaseIndent();
    
    printIndent();
    result_ += "参数:\n";
    increaseIndent();
    for (auto& param : node.parameters) {
        if (param) {
            param->accept(*this);
        }
    }
    decreaseIndent();
    
    printIndent();
    result_ += "函数体:\n";
    increaseIndent();
    if (node.body) {
        node.body->accept(*this);
    }
    decreaseIndent();
    decreaseIndent();
}

void ASTPrintVisitor::visitClassDeclarationNode(ClassDeclarationNode& node) {
    printIndent();
    std::string superclass = node.superclass.getValue().empty() ? "" : " extends " + node.superclass.getValue();
    result_ += "类声明: " + node.name.getValue() + superclass + "\n";
    increaseIndent();
    for (auto& member : node.members) {
        if (member) {
            member->accept(*this);
        }
    }
    decreaseIndent();
}

void ASTPrintVisitor::visitBlockStatementNode(BlockStatementNode& node) {
    printIndent();
    result_ += "块语句\n";
    increaseIndent();
    for (auto& stmt : node.statements) {
        if (stmt) {
            stmt->accept(*this);
        }
    }
    decreaseIndent();
}

void ASTPrintVisitor::visitIfStatementNode(IfStatementNode& node) {
    printIndent();
    result_ += "if语句\n";
    increaseIndent();
    
    printIndent();
    result_ += "条件:\n";
    increaseIndent();
    if (node.condition) {
        node.condition->accept(*this);
    }
    decreaseIndent();
    
    printIndent();
    result_ += "then分支:\n";
    increaseIndent();
    if (node.then_branch) {
        node.then_branch->accept(*this);
    }
    decreaseIndent();
    
    if (node.else_branch) {
        printIndent();
        result_ += "else分支:\n";
        increaseIndent();
        node.else_branch->accept(*this);
        decreaseIndent();
    }
    decreaseIndent();
}

void ASTPrintVisitor::visitWhileStatementNode(WhileStatementNode& node) {
    printIndent();
    result_ += "while语句\n";
    increaseIndent();
    
    printIndent();
    result_ += "条件:\n";
    increaseIndent();
    if (node.condition) {
        node.condition->accept(*this);
    }
    decreaseIndent();
    
    printIndent();
    result_ += "循环体:\n";
    increaseIndent();
    if (node.body) {
        node.body->accept(*this);
    }
    decreaseIndent();
    decreaseIndent();
}

void ASTPrintVisitor::visitForStatementNode(ForStatementNode& node) {
    printIndent();
    result_ += "for语句\n";
    increaseIndent();
    
    if (node.initializer) {
        printIndent();
        result_ += "初始化:\n";
        increaseIndent();
        node.initializer->accept(*this);
        decreaseIndent();
    }
    
    if (node.condition) {
        printIndent();
        result_ += "条件:\n";
        increaseIndent();
        node.condition->accept(*this);
        decreaseIndent();
    }
    
    if (node.increment) {
        printIndent();
        result_ += "递增:\n";
        increaseIndent();
        node.increment->accept(*this);
        decreaseIndent();
    }
    
    printIndent();
    result_ += "循环体:\n";
    increaseIndent();
    if (node.body) {
        node.body->accept(*this);
    }
    decreaseIndent();
    decreaseIndent();
}

void ASTPrintVisitor::visitReturnStatementNode(ReturnStatementNode& node) {
    printIndent();
    result_ += "return语句\n";
    increaseIndent();
    if (node.value) {
        node.value->accept(*this);
    }
    decreaseIndent();
}

void ASTPrintVisitor::visitBreakStatementNode(BreakStatementNode& node) {
    printIndent();
    result_ += "break语句\n";
}

void ASTPrintVisitor::visitContinueStatementNode(ContinueStatementNode& node) {
    printIndent();
    result_ += "continue语句\n";
}

} // namespace starry