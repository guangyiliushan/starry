#include "starry/codegen/LLVMEmitter.h"
#include "starry/AST.h"
#include <iostream>
#include <stdexcept>

namespace starry {
namespace codegen {

LLVMEmitter::LLVMEmitter() 
    : context_(std::make_unique<llvm::LLVMContext>()),
      module_(std::make_unique<llvm::Module>("starry_module", *context_)),
      builder_(std::make_unique<llvm::IRBuilder<>>(*context_)) {
    
    // 初始化LLVM类型映射
    initializeTypes();
}

LLVMEmitter::~LLVMEmitter() = default;

void LLVMEmitter::initializeTypes() {
    // 映射Starry类型到LLVM类型
    typeMap_["int"] = llvm::Type::getInt32Ty(*context_);
    typeMap_["float"] = llvm::Type::getFloatTy(*context_);
    typeMap_["bool"] = llvm::Type::getInt1Ty(*context_);
    typeMap_["void"] = llvm::Type::getVoidTy(*context_);
    typeMap_["string"] = llvm::Type::getInt8PtrTy(*context_);
}

llvm::Module* LLVMEmitter::emitProgram(ast::Program* program) {
    if (!program) {
        throw std::runtime_error("空程序指针");
    }
    
    // 遍历程序中的所有语句生成LLVM IR
    for (const auto& stmt : program->getStatements()) {
        emitStatement(stmt.get());
    }
    
    return module_.get();
}

llvm::Value* LLVMEmitter::emitExpression(ast::Expression* expr) {
    if (!expr) {
        return nullptr;
    }
    
    switch (expr->getType()) {
        case ast::ExpressionType::LITERAL:
            return emitLiteralExpression(static_cast<ast::LiteralExpression*>(expr));
        case ast::ExpressionType::IDENTIFIER:
            return emitIdentifierExpression(static_cast<ast::IdentifierExpression*>(expr));
        case ast::ExpressionType::BINARY:
            return emitBinaryExpression(static_cast<ast::BinaryExpression*>(expr));
        case ast::ExpressionType::UNARY:
            return emitUnaryExpression(static_cast<ast::UnaryExpression*>(expr));
        case ast::ExpressionType::CALL:
            return emitCallExpression(static_cast<ast::CallExpression*>(expr));
        case ast::ExpressionType::ASSIGNMENT:
            return emitAssignmentExpression(static_cast<ast::AssignmentExpression*>(expr));
        default:
            throw std::runtime_error("未知的表达式类型");
    }
}

void LLVMEmitter::emitStatement(ast::Statement* stmt) {
    if (!stmt) {
        return;
    }
    
    switch (stmt->getType()) {
        case ast::StatementType::EXPRESSION:
            emitExpressionStatement(static_cast<ast::ExpressionStatement*>(stmt));
            break;
        case ast::StatementType::VARIABLE_DECLARATION:
            emitVariableDeclaration(static_cast<ast::VariableDeclaration*>(stmt));
            break;
        case ast::StatementType::BLOCK:
            emitBlockStatement(static_cast<ast::BlockStatement*>(stmt));
            break;
        case ast::StatementType::IF:
            emitIfStatement(static_cast<ast::IfStatement*>(stmt));
            break;
        case ast::StatementType::WHILE:
            emitWhileStatement(static_cast<ast::WhileStatement*>(stmt));
            break;
        case ast::StatementType::FOR:
            emitForStatement(static_cast<ast::ForStatement*>(stmt));
            break;
        case ast::StatementType::RETURN:
            emitReturnStatement(static_cast<ast::ReturnStatement*>(stmt));
            break;
        case ast::StatementType::FUNCTION_DECLARATION:
            emitFunctionDeclaration(static_cast<ast::FunctionDeclaration*>(stmt));
            break;
        case ast::StatementType::BREAK:
            emitBreakStatement();
            break;
        case ast::StatementType::CONTINUE:
            emitContinueStatement();
            break;
        default:
            throw std::runtime_error("未知的语句类型");
    }
}

llvm::Value* LLVMEmitter::emitLiteralExpression(ast::LiteralExpression* expr) {
    switch (expr->getLiteralType()) {
        case ast::LiteralType::INTEGER:
            return llvm::ConstantInt::get(typeMap_["int"], std::stoi(expr->getValue()));
        case ast::LiteralType::FLOAT:
            return llvm::ConstantFP::get(typeMap_["float"], std::stof(expr->getValue()));
        case ast::LiteralType::BOOLEAN:
            return llvm::ConstantInt::get(typeMap_["bool"], expr->getValue() == "true" ? 1 : 0);
        case ast::LiteralType::STRING:
            return builder_->CreateGlobalStringPtr(expr->getValue());
        default:
            throw std::runtime_error("未知的字面量类型");
    }
}

llvm::Value* LLVMEmitter::emitIdentifierExpression(ast::IdentifierExpression* expr) {
    const std::string& name = expr->getName();
    
    auto it = variables_.find(name);
    if (it == variables_.end()) {
        throw std::runtime_error("未定义的变量: " + name);
    }
    
    return builder_->CreateLoad(it->second->getAllocatedType(), it->second, name);
}

llvm::Value* LLVMEmitter::emitBinaryExpression(ast::BinaryExpression* expr) {
    llvm::Value* left = emitExpression(expr->getLeft());
    llvm::Value* right = emitExpression(expr->getRight());
    
    if (!left || !right) {
        throw std::runtime_error("二元表达式操作数生成失败");
    }
    
    switch (expr->getOperator()) {
        case ast::BinaryOperator::ADD:
            return builder_->CreateAdd(left, right, "addtmp");
        case ast::BinaryOperator::SUBTRACT:
            return builder_->CreateSub(left, right, "subtmp");
        case ast::BinaryOperator::MULTIPLY:
            return builder_->CreateMul(left, right, "multmp");
        case ast::BinaryOperator::DIVIDE:
            return builder_->CreateSDiv(left, right, "divtmp");
        case ast::BinaryOperator::EQUAL:
            return builder_->CreateICmpEQ(left, right, "eqtmp");
        case ast::BinaryOperator::NOT_EQUAL:
            return builder_->CreateICmpNE(left, right, "netmp");
        case ast::BinaryOperator::LESS:
            return builder_->CreateICmpSLT(left, right, "lttmp");
        case ast::BinaryOperator::LESS_EQUAL:
            return builder_->CreateICmpSLE(left, right, "letmp");
        case ast::BinaryOperator::GREATER:
            return builder_->CreateICmpSGT(left, right, "gttmp");
        case ast::BinaryOperator::GREATER_EQUAL:
            return builder_->CreateICmpSGE(left, right, "getmp");
        case ast::BinaryOperator::LOGICAL_AND:
            return builder_->CreateAnd(left, right, "andtmp");
        case ast::BinaryOperator::LOGICAL_OR:
            return builder_->CreateOr(left, right, "ortmp");
        default:
            throw std::runtime_error("未知的二元操作符");
    }
}

llvm::Value* LLVMEmitter::emitUnaryExpression(ast::UnaryExpression* expr) {
    llvm::Value* operand = emitExpression(expr->getOperand());
    
    if (!operand) {
        throw std::runtime_error("一元表达式操作数生成失败");
    }
    
    switch (expr->getOperator()) {
        case ast::UnaryOperator::MINUS:
            return builder_->CreateNeg(operand, "negtmp");
        case ast::UnaryOperator::LOGICAL_NOT:
            return builder_->CreateNot(operand, "nottmp");
        default:
            throw std::runtime_error("未知的一元操作符");
    }
}

llvm::Value* LLVMEmitter::emitCallExpression(ast::CallExpression* expr) {
    const std::string& functionName = expr->getFunctionName();
    
    llvm::Function* function = module_->getFunction(functionName);
    if (!function) {
        throw std::runtime_error("未定义的函数: " + functionName);
    }
    
    // 生成参数
    std::vector<llvm::Value*> args;
    for (const auto& arg : expr->getArguments()) {
        llvm::Value* argValue = emitExpression(arg.get());
        if (!argValue) {
            throw std::runtime_error("函数参数生成失败");
        }
        args.push_back(argValue);
    }
    
    return builder_->CreateCall(function, args, "calltmp");
}

llvm::Value* LLVMEmitter::emitAssignmentExpression(ast::AssignmentExpression* expr) {
    llvm::Value* value = emitExpression(expr->getValue());
    if (!value) {
        throw std::runtime_error("赋值表达式值生成失败");
    }
    
    // 获取目标变量
    ast::IdentifierExpression* target = static_cast<ast::IdentifierExpression*>(expr->getTarget());
    const std::string& name = target->getName();
    
    auto it = variables_.find(name);
    if (it == variables_.end()) {
        throw std::runtime_error("未定义的变量: " + name);
    }
    
    builder_->CreateStore(value, it->second);
    return value;
}

void LLVMEmitter::emitExpressionStatement(ast::ExpressionStatement* stmt) {
    emitExpression(stmt->getExpression());
}

void LLVMEmitter::emitVariableDeclaration(ast::VariableDeclaration* stmt) {
    const std::string& name = stmt->getName();
    
    // 创建alloca指令
    llvm::AllocaInst* alloca = builder_->CreateAlloca(typeMap_["int"], nullptr, name);
    variables_[name] = alloca;
    
    // 如果有初始化器，生成初始化代码
    if (stmt->getInitializer()) {
        llvm::Value* initValue = emitExpression(stmt->getInitializer());
        if (initValue) {
            builder_->CreateStore(initValue, alloca);
        }
    }
}

void LLVMEmitter::emitBlockStatement(ast::BlockStatement* stmt) {
    for (const auto& statement : stmt->getStatements()) {
        emitStatement(statement.get());
    }
}

void LLVMEmitter::emitIfStatement(ast::IfStatement* stmt) {
    llvm::Value* condition = emitExpression(stmt->getCondition());
    if (!condition) {
        throw std::runtime_error("if语句条件生成失败");
    }
    
    llvm::Function* function = builder_->GetInsertBlock()->getParent();
    
    // 创建基本块
    llvm::BasicBlock* thenBB = llvm::BasicBlock::Create(*context_, "then", function);
    llvm::BasicBlock* elseBB = llvm::BasicBlock::Create(*context_, "else");
    llvm::BasicBlock* mergeBB = llvm::BasicBlock::Create(*context_, "ifcont");
    
    // 创建条件分支
    builder_->CreateCondBr(condition, thenBB, elseBB);
    
    // 生成then分支
    builder_->SetInsertPoint(thenBB);
    emitStatement(stmt->getThenStatement());
    builder_->CreateBr(mergeBB);
    
    // 生成else分支
    function->getBasicBlockList().push_back(elseBB);
    builder_->SetInsertPoint(elseBB);
    if (stmt->getElseStatement()) {
        emitStatement(stmt->getElseStatement());
    }
    builder_->CreateBr(mergeBB);
    
    // 合并点
    function->getBasicBlockList().push_back(mergeBB);
    builder_->SetInsertPoint(mergeBB);
}

void LLVMEmitter::emitWhileStatement(ast::WhileStatement* stmt) {
    llvm::Function* function = builder_->GetInsertBlock()->getParent();
    
    // 创建基本块
    llvm::BasicBlock* condBB = llvm::BasicBlock::Create(*context_, "whilecond", function);
    llvm::BasicBlock* bodyBB = llvm::BasicBlock::Create(*context_, "whilebody");
    llvm::BasicBlock* afterBB = llvm::BasicBlock::Create(*context_, "afterwhile");
    
    // 跳转到条件检查
    builder_->CreateBr(condBB);
    
    // 生成条件检查
    builder_->SetInsertPoint(condBB);
    llvm::Value* condition = emitExpression(stmt->getCondition());
    builder_->CreateCondBr(condition, bodyBB, afterBB);
    
    // 生成循环体
    function->getBasicBlockList().push_back(bodyBB);
    builder_->SetInsertPoint(bodyBB);
    emitStatement(stmt->getBody());
    builder_->CreateBr(condBB);
    
    // 循环后继续执行
    function->getBasicBlockList().push_back(afterBB);
    builder_->SetInsertPoint(afterBB);
}

void LLVMEmitter::emitForStatement(ast::ForStatement* stmt) {
    // 简化实现：将for循环转换为while循环
    if (stmt->getInit()) {
        emitStatement(stmt->getInit());
    }
    
    llvm::Function* function = builder_->GetInsertBlock()->getParent();
    
    llvm::BasicBlock* condBB = llvm::BasicBlock::Create(*context_, "forcond", function);
    llvm::BasicBlock* bodyBB = llvm::BasicBlock::Create(*context_, "forbody");
    llvm::BasicBlock* updateBB = llvm::BasicBlock::Create(*context_, "forupdate");
    llvm::BasicBlock* afterBB = llvm::BasicBlock::Create(*context_, "afterfor");
    
    builder_->CreateBr(condBB);
    
    // 条件检查
    builder_->SetInsertPoint(condBB);
    if (stmt->getCondition()) {
        llvm::Value* condition = emitExpression(stmt->getCondition());
        builder_->CreateCondBr(condition, bodyBB, afterBB);
    } else {
        builder_->CreateBr(bodyBB);
    }
    
    // 循环体
    function->getBasicBlockList().push_back(bodyBB);
    builder_->SetInsertPoint(bodyBB);
    emitStatement(stmt->getBody());
    builder_->CreateBr(updateBB);
    
    // 更新语句
    function->getBasicBlockList().push_back(updateBB);
    builder_->SetInsertPoint(updateBB);
    if (stmt->getUpdate()) {
        emitExpression(stmt->getUpdate());
    }
    builder_->CreateBr(condBB);
    
    // 循环后
    function->getBasicBlockList().push_back(afterBB);
    builder_->SetInsertPoint(afterBB);
}

void LLVMEmitter::emitReturnStatement(ast::ReturnStatement* stmt) {
    if (stmt->getValue()) {
        llvm::Value* returnValue = emitExpression(stmt->getValue());
        builder_->CreateRet(returnValue);
    } else {
        builder_->CreateRetVoid();
    }
}

void LLVMEmitter::emitFunctionDeclaration(ast::FunctionDeclaration* stmt) {
    const std::string& name = stmt->getName();
    const auto& parameters = stmt->getParameters();
    
    // 创建函数类型
    std::vector<llvm::Type*> paramTypes(parameters.size(), typeMap_["int"]);
    llvm::FunctionType* functionType = llvm::FunctionType::get(typeMap_["void"], paramTypes, false);
    
    // 创建函数
    llvm::Function* function = llvm::Function::Create(
        functionType, llvm::Function::ExternalLinkage, name, module_.get());
    
    // 设置参数名称
    unsigned idx = 0;
    for (auto& arg : function->args()) {
        arg.setName(parameters[idx++]);
    }
    
    // 创建函数体基本块
    llvm::BasicBlock* entryBB = llvm::BasicBlock::Create(*context_, "entry", function);
    builder_->SetInsertPoint(entryBB);
    
    // 为参数创建alloca
    for (auto& arg : function->args()) {
        llvm::AllocaInst* alloca = builder_->CreateAlloca(arg.getType(), nullptr, arg.getName());
        builder_->CreateStore(&arg, alloca);
        variables_[std::string(arg.getName())] = alloca;
    }
    
    // 生成函数体
    emitStatement(stmt->getBody());
    
    // 如果函数没有显式返回，添加void返回
    if (!builder_->GetInsertBlock()->getTerminator()) {
        builder_->CreateRetVoid();
    }
}

void LLVMEmitter::emitBreakStatement() {
    // 简化实现：跳转到循环后的基本块
    // 实际实现需要维护循环上下文栈
    throw std::runtime_error("break语句暂未完全实现");
}

void LLVMEmitter::emitContinueStatement() {
    // 简化实现：跳转到循环条件检查
    // 实际实现需要维护循环上下文栈
    throw std::runtime_error("continue语句暂未完全实现");
}

std::string LLVMEmitter::getIR() const {
    std::string ir;
    llvm::raw_string_ostream stream(ir);
    module_->print(stream, nullptr);
    return ir;
}

void LLVMEmitter::dumpIR() const {
    module_->print(llvm::errs(), nullptr);
}

} // namespace codegen
} // namespace starry