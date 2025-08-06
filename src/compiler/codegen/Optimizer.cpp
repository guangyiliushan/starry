#include "starry/codegen/Optimizer.h"
#include "starry/ast/Expression.h"
#include "starry/ast/Statement.h"
#include <algorithm>
#include <unordered_set>
#include <queue>

namespace starry {
namespace codegen {

// Optimizer 实现
Optimizer::Optimizer() : optimizationLevel_(OptimizationLevel::O2) {}

void Optimizer::setOptimizationLevel(OptimizationLevel level) {
    optimizationLevel_ = level;
}

OptimizationLevel Optimizer::getOptimizationLevel() const {
    return optimizationLevel_;
}

std::unique_ptr<ast::ASTNode> Optimizer::optimize(std::unique_ptr<ast::ASTNode> node) {
    if (!node) return nullptr;
    
    switch (optimizationLevel_) {
        case OptimizationLevel::O0:
            // 无优化
            return node;
        case OptimizationLevel::O1:
            return applyBasicOptimizations(std::move(node));
        case OptimizationLevel::O2:
            node = applyBasicOptimizations(std::move(node));
            return applyAdvancedOptimizations(std::move(node));
        case OptimizationLevel::O3:
            node = applyBasicOptimizations(std::move(node));
            node = applyAdvancedOptimizations(std::move(node));
            return applyAggressiveOptimizations(std::move(node));
        default:
            return node;
    }
}

std::unique_ptr<ast::ASTNode> Optimizer::applyBasicOptimizations(std::unique_ptr<ast::ASTNode> node) {
    // 常量折叠
    node = constantFolding(std::move(node));
    
    // 死代码消除
    node = deadCodeElimination(std::move(node));
    
    // 简单的代数简化
    node = algebraicSimplification(std::move(node));
    
    return node;
}

std::unique_ptr<ast::ASTNode> Optimizer::applyAdvancedOptimizations(std::unique_ptr<ast::ASTNode> node) {
    // 公共子表达式消除
    node = commonSubexpressionElimination(std::move(node));
    
    // 循环优化
    node = loopOptimization(std::move(node));
    
    // 内联优化
    node = inlineOptimization(std::move(node));
    
    return node;
}

std::unique_ptr<ast::ASTNode> Optimizer::applyAggressiveOptimizations(std::unique_ptr<ast::ASTNode> node) {
    // 向量化
    node = vectorization(std::move(node));
    
    // 循环展开
    node = loopUnrolling(std::move(node));
    
    // 函数特化
    node = functionSpecialization(std::move(node));
    
    return node;
}

// ConstantFolder 实现
ConstantFolder::ConstantFolder() {}

std::unique_ptr<ast::ASTNode> ConstantFolder::fold(std::unique_ptr<ast::ASTNode> node) {
    if (!node) return nullptr;
    
    // 根据节点类型进行常量折叠
    if (auto expr = dynamic_cast<ast::Expression*>(node.get())) {
        return foldExpression(std::unique_ptr<ast::Expression>(
            static_cast<ast::Expression*>(node.release())));
    }
    
    return node;
}

std::unique_ptr<ast::Expression> ConstantFolder::foldExpression(std::unique_ptr<ast::Expression> expr) {
    if (!expr) return nullptr;
    
    // 处理二元表达式
    if (auto binExpr = dynamic_cast<ast::BinaryExpression*>(expr.get())) {
        return foldBinaryExpression(std::unique_ptr<ast::BinaryExpression>(
            static_cast<ast::BinaryExpression*>(expr.release())));
    }
    
    // 处理一元表达式
    if (auto unaryExpr = dynamic_cast<ast::UnaryExpression*>(expr.get())) {
        return foldUnaryExpression(std::unique_ptr<ast::UnaryExpression>(
            static_cast<ast::UnaryExpression*>(expr.release())));
    }
    
    return expr;
}

std::unique_ptr<ast::Expression> ConstantFolder::foldBinaryExpression(
    std::unique_ptr<ast::BinaryExpression> expr) {
    
    if (!expr) return nullptr;
    
    // 先折叠子表达式
    auto left = foldExpression(std::unique_ptr<ast::Expression>(expr->getLeft()));
    auto right = foldExpression(std::unique_ptr<ast::Expression>(expr->getRight()));
    
    // 检查是否都是字面量
    auto leftLiteral = dynamic_cast<ast::LiteralExpression*>(left.get());
    auto rightLiteral = dynamic_cast<ast::LiteralExpression*>(right.get());
    
    if (leftLiteral && rightLiteral) {
        // 执行常量计算
        return evaluateConstantBinaryOperation(expr->getOperator(), leftLiteral, rightLiteral);
    }
    
    // 更新子表达式并返回
    expr->setLeft(std::move(left));
    expr->setRight(std::move(right));
    return std::move(expr);
}

std::unique_ptr<ast::Expression> ConstantFolder::foldUnaryExpression(
    std::unique_ptr<ast::UnaryExpression> expr) {
    
    if (!expr) return nullptr;
    
    // 先折叠操作数
    auto operand = foldExpression(std::unique_ptr<ast::Expression>(expr->getOperand()));
    
    // 检查是否是字面量
    if (auto literal = dynamic_cast<ast::LiteralExpression*>(operand.get())) {
        // 执行常量计算
        return evaluateConstantUnaryOperation(expr->getOperator(), literal);
    }
    
    // 更新操作数并返回
    expr->setOperand(std::move(operand));
    return std::move(expr);
}

std::unique_ptr<ast::LiteralExpression> ConstantFolder::evaluateConstantBinaryOperation(
    const std::string& op, ast::LiteralExpression* left, ast::LiteralExpression* right) {
    
    // 这里简化实现，实际需要根据具体的字面量类型和操作符进行计算
    // 示例：整数加法
    if (op == "+" && left->getType() == "int" && right->getType() == "int") {
        int leftVal = std::stoi(left->getValue());
        int rightVal = std::stoi(right->getValue());
        int result = leftVal + rightVal;
        return std::make_unique<ast::LiteralExpression>(std::to_string(result), "int");
    }
    
    // 其他操作符的实现...
    return nullptr;
}

std::unique_ptr<ast::LiteralExpression> ConstantFolder::evaluateConstantUnaryOperation(
    const std::string& op, ast::LiteralExpression* operand) {
    
    // 示例：整数取负
    if (op == "-" && operand->getType() == "int") {
        int val = std::stoi(operand->getValue());
        int result = -val;
        return std::make_unique<ast::LiteralExpression>(std::to_string(result), "int");
    }
    
    // 其他操作符的实现...
    return nullptr;
}

// DeadCodeEliminator 实现
DeadCodeEliminator::DeadCodeEliminator() {}

std::unique_ptr<ast::ASTNode> DeadCodeEliminator::eliminate(std::unique_ptr<ast::ASTNode> node) {
    if (!node) return nullptr;
    
    // 分析活跃变量
    analyzeReachability(node.get());
    
    // 移除死代码
    return removeDeadCode(std::move(node));
}

void DeadCodeEliminator::analyzeReachability(ast::ASTNode* node) {
    if (!node) return;
    
    reachableNodes_.clear();
    std::queue<ast::ASTNode*> workList;
    
    // 从根节点开始标记可达节点
    workList.push(node);
    reachableNodes_.insert(node);
    
    while (!workList.empty()) {
        ast::ASTNode* current = workList.front();
        workList.pop();
        
        // 访问所有子节点
        for (auto& child : current->getChildren()) {
            if (child && reachableNodes_.find(child.get()) == reachableNodes_.end()) {
                reachableNodes_.insert(child.get());
                workList.push(child.get());
            }
        }
    }
}

std::unique_ptr<ast::ASTNode> DeadCodeEliminator::removeDeadCode(std::unique_ptr<ast::ASTNode> node) {
    if (!node) return nullptr;
    
    // 如果节点不可达，返回nullptr
    if (reachableNodes_.find(node.get()) == reachableNodes_.end()) {
        return nullptr;
    }
    
    // 递归处理子节点
    auto children = node->getChildren();
    for (auto& child : children) {
        child = removeDeadCode(std::move(child));
    }
    
    return node;
}

// LoopOptimizer 实现
LoopOptimizer::LoopOptimizer() {}

std::unique_ptr<ast::ASTNode> LoopOptimizer::optimize(std::unique_ptr<ast::ASTNode> node) {
    if (!node) return nullptr;
    
    // 循环不变量外提
    node = loopInvariantCodeMotion(std::move(node));
    
    // 循环强度削减
    node = strengthReduction(std::move(node));
    
    // 循环合并
    node = loopFusion(std::move(node));
    
    return node;
}

std::unique_ptr<ast::ASTNode> LoopOptimizer::loopInvariantCodeMotion(std::unique_ptr<ast::ASTNode> node) {
    // 简化实现：识别循环不变量并外提
    if (auto whileStmt = dynamic_cast<ast::WhileStatement*>(node.get())) {
        // 分析循环体中的不变量
        analyzeLoopInvariants(whileStmt);
        
        // 外提不变量
        return hoistInvariants(std::unique_ptr<ast::WhileStatement>(
            static_cast<ast::WhileStatement*>(node.release())));
    }
    
    return node;
}

std::unique_ptr<ast::ASTNode> LoopOptimizer::strengthReduction(std::unique_ptr<ast::ASTNode> node) {
    // 简化实现：将乘法转换为加法
    return node;
}

std::unique_ptr<ast::ASTNode> LoopOptimizer::loopFusion(std::unique_ptr<ast::ASTNode> node) {
    // 简化实现：合并相邻的相似循环
    return node;
}

void LoopOptimizer::analyzeLoopInvariants(ast::WhileStatement* whileStmt) {
    // 分析循环体中的表达式，识别不变量
    invariants_.clear();
    // 实际实现需要数据流分析
}

std::unique_ptr<ast::WhileStatement> LoopOptimizer::hoistInvariants(
    std::unique_ptr<ast::WhileStatement> whileStmt) {
    // 将识别的不变量移到循环外
    return whileStmt;
}

// InlineOptimizer 实现
InlineOptimizer::InlineOptimizer() : maxInlineDepth_(10), maxInlineSize_(100) {}

std::unique_ptr<ast::ASTNode> InlineOptimizer::optimize(std::unique_ptr<ast::ASTNode> node) {
    if (!node) return nullptr;
    
    // 分析函数调用
    analyzeFunctionCalls(node.get());
    
    // 执行内联
    return performInlining(std::move(node));
}

void InlineOptimizer::analyzeFunctionCalls(ast::ASTNode* node) {
    // 收集所有函数调用信息
    callSites_.clear();
    // 实际实现需要遍历AST收集调用点
}

std::unique_ptr<ast::ASTNode> InlineOptimizer::performInlining(std::unique_ptr<ast::ASTNode> node) {
    // 对符合条件的函数调用执行内联
    return node;
}

bool InlineOptimizer::shouldInline(ast::FunctionDeclaration* func, ast::CallExpression* call) {
    // 判断是否应该内联
    if (!func || !call) return false;
    
    // 检查函数大小
    if (estimateFunctionSize(func) > maxInlineSize_) {
        return false;
    }
    
    // 检查递归深度
    if (getInlineDepth(call) > maxInlineDepth_) {
        return false;
    }
    
    // 其他启发式规则
    return true;
}

size_t InlineOptimizer::estimateFunctionSize(ast::FunctionDeclaration* func) {
    // 估算函数大小（节点数量）
    if (!func || !func->getBody()) return 0;
    
    // 简化实现：计算语句数量
    size_t size = 0;
    // 实际需要遍历函数体计算复杂度
    return size;
}

size_t InlineOptimizer::getInlineDepth(ast::CallExpression* call) {
    // 获取当前内联深度
    return 0; // 简化实现
}

// 辅助函数实现
std::unique_ptr<ast::ASTNode> Optimizer::constantFolding(std::unique_ptr<ast::ASTNode> node) {
    ConstantFolder folder;
    return folder.fold(std::move(node));
}

std::unique_ptr<ast::ASTNode> Optimizer::deadCodeElimination(std::unique_ptr<ast::ASTNode> node) {
    DeadCodeEliminator eliminator;
    return eliminator.eliminate(std::move(node));
}

std::unique_ptr<ast::ASTNode> Optimizer::algebraicSimplification(std::unique_ptr<ast::ASTNode> node) {
    // 简化实现：基本代数简化
    // 例如：x + 0 -> x, x * 1 -> x, x * 0 -> 0
    return node;
}

std::unique_ptr<ast::ASTNode> Optimizer::commonSubexpressionElimination(std::unique_ptr<ast::ASTNode> node) {
    // 简化实现：公共子表达式消除
    return node;
}

std::unique_ptr<ast::ASTNode> Optimizer::loopOptimization(std::unique_ptr<ast::ASTNode> node) {
    LoopOptimizer optimizer;
    return optimizer.optimize(std::move(node));
}

std::unique_ptr<ast::ASTNode> Optimizer::inlineOptimization(std::unique_ptr<ast::ASTNode> node) {
    InlineOptimizer optimizer;
    return optimizer.optimize(std::move(node));
}

std::unique_ptr<ast::ASTNode> Optimizer::vectorization(std::unique_ptr<ast::ASTNode> node) {
    // 简化实现：向量化优化
    return node;
}

std::unique_ptr<ast::ASTNode> Optimizer::loopUnrolling(std::unique_ptr<ast::ASTNode> node) {
    // 简化实现：循环展开
    return node;
}

std::unique_ptr<ast::ASTNode> Optimizer::functionSpecialization(std::unique_ptr<ast::ASTNode> node) {
    // 简化实现：函数特化
    return node;
}

} // namespace codegen
} // namespace starry