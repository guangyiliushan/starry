#include "starry/codegen/LLVMEmitter.h"
#include "starry/AST.h"
#include <llvm/IR/IRBuilder.h>
#include <llvm/IR/LLVMContext.h>
#include <llvm/IR/Module.h>
#include <llvm/IR/Function.h>
#include <llvm/IR/BasicBlock.h>
#include <llvm/IR/Type.h>
#include <llvm/IR/Value.h>
#include <llvm/IR/Constants.h>
#include <unordered_map>
#include <memory>

namespace starry {
namespace codegen {

// IR构建器实现
class IRBuilder {
private:
    std::unique_ptr<llvm::LLVMContext> context;
    std::unique_ptr<llvm::Module> module;
    std::unique_ptr<llvm::IRBuilder<>> builder;
    
    // 符号表
    std::unordered_map<std::string, llvm::Value*> namedValues;
    std::unordered_map<std::string, llvm::Function*> functions;
    
    // 类型映射
    std::unordered_map<std::string, llvm::Type*> typeMap;
    
public:
    IRBuilder(const std::string& moduleName) {
        context = std::make_unique<llvm::LLVMContext>();
        module = std::make_unique<llvm::Module>(moduleName, *context);
        builder = std::make_unique<llvm::IRBuilder<>>(*context);
        
        initializeBuiltinTypes();
    }
    
    // 初始化内置类型
    void initializeBuiltinTypes() {
        typeMap["void"] = llvm::Type::getVoidTy(*context);
        typeMap["bool"] = llvm::Type::getInt1Ty(*context);
        typeMap["int"] = llvm::Type::getInt32Ty(*context);
        typeMap["long"] = llvm::Type::getInt64Ty(*context);
        typeMap["float"] = llvm::Type::getFloatTy(*context);
        typeMap["double"] = llvm::Type::getDoubleTy(*context);
        typeMap["string"] = llvm::Type::getInt8PtrTy(*context);
    }
    
    // 获取LLVM类型
    llvm::Type* getLLVMType(const std::string& typeName) {
        auto it = typeMap.find(typeName);
        if (it != typeMap.end()) {
            return it->second;
        }
        
        // 默认返回int类型
        return llvm::Type::getInt32Ty(*context);
    }
    
    // 创建函数
    llvm::Function* createFunction(const std::string& name, 
                                  const std::string& returnType,
                                  const std::vector<std::pair<std::string, std::string>>& params) {
        // 构建参数类型列表
        std::vector<llvm::Type*> paramTypes;
        for (const auto& param : params) {
            paramTypes.push_back(getLLVMType(param.second));
        }
        
        // 创建函数类型
        llvm::Type* retType = getLLVMType(returnType);
        llvm::FunctionType* funcType = llvm::FunctionType::get(retType, paramTypes, false);
        
        // 创建函数
        llvm::Function* func = llvm::Function::Create(
            funcType, llvm::Function::ExternalLinkage, name, module.get());
        
        // 设置参数名称
        auto argIt = func->arg_begin();
        for (const auto& param : params) {
            argIt->setName(param.first);
            ++argIt;
        }
        
        functions[name] = func;
        return func;
    }
    
    // 创建基本块
    llvm::BasicBlock* createBasicBlock(const std::string& name, llvm::Function* func) {
        return llvm::BasicBlock::Create(*context, name, func);
    }
    
    // 设置插入点
    void setInsertPoint(llvm::BasicBlock* block) {
        builder->SetInsertPoint(block);
    }
    
    // 创建变量分配
    llvm::AllocaInst* createAlloca(const std::string& name, const std::string& type) {
        llvm::Type* llvmType = getLLVMType(type);
        llvm::AllocaInst* alloca = builder->CreateAlloca(llvmType, nullptr, name);
        namedValues[name] = alloca;
        return alloca;
    }
    
    // 创建存储指令
    llvm::StoreInst* createStore(llvm::Value* value, llvm::Value* ptr) {
        return builder->CreateStore(value, ptr);
    }
    
    // 创建加载指令
    llvm::LoadInst* createLoad(llvm::Value* ptr, const std::string& name = "") {
        return builder->CreateLoad(ptr->getType()->getPointerElementType(), ptr, name);
    }
    
    // 创建常量
    llvm::Constant* createConstant(const std::string& type, const std::string& value) {
        if (type == "int") {
            return llvm::ConstantInt::get(*context, llvm::APInt(32, std::stoi(value), true));
        } else if (type == "float") {
            return llvm::ConstantFP::get(*context, llvm::APFloat(std::stof(value)));
        } else if (type == "bool") {
            bool boolValue = (value == "true" || value == "1");
            return llvm::ConstantInt::get(*context, llvm::APInt(1, boolValue ? 1 : 0, false));
        } else if (type == "string") {
            return builder->CreateGlobalStringPtr(value);
        }
        
        return nullptr;
    }
    
    // 创建二元操作
    llvm::Value* createBinaryOp(const std::string& op, llvm::Value* left, llvm::Value* right) {
        if (op == "+") {
            if (left->getType()->isIntegerTy()) {
                return builder->CreateAdd(left, right, "addtmp");
            } else if (left->getType()->isFloatingPointTy()) {
                return builder->CreateFAdd(left, right, "addtmp");
            }
        } else if (op == "-") {
            if (left->getType()->isIntegerTy()) {
                return builder->CreateSub(left, right, "subtmp");
            } else if (left->getType()->isFloatingPointTy()) {
                return builder->CreateFSub(left, right, "subtmp");
            }
        } else if (op == "*") {
            if (left->getType()->isIntegerTy()) {
                return builder->CreateMul(left, right, "multmp");
            } else if (left->getType()->isFloatingPointTy()) {
                return builder->CreateFMul(left, right, "multmp");
            }
        } else if (op == "/") {
            if (left->getType()->isIntegerTy()) {
                return builder->CreateSDiv(left, right, "divtmp");
            } else if (left->getType()->isFloatingPointTy()) {
                return builder->CreateFDiv(left, right, "divtmp");
            }
        } else if (op == "%") {
            if (left->getType()->isIntegerTy()) {
                return builder->CreateSRem(left, right, "modtmp");
            }
        } else if (op == "==") {
            if (left->getType()->isIntegerTy()) {
                return builder->CreateICmpEQ(left, right, "eqtmp");
            } else if (left->getType()->isFloatingPointTy()) {
                return builder->CreateFCmpOEQ(left, right, "eqtmp");
            }
        } else if (op == "!=") {
            if (left->getType()->isIntegerTy()) {
                return builder->CreateICmpNE(left, right, "netmp");
            } else if (left->getType()->isFloatingPointTy()) {
                return builder->CreateFCmpONE(left, right, "netmp");
            }
        } else if (op == "<") {
            if (left->getType()->isIntegerTy()) {
                return builder->CreateICmpSLT(left, right, "lttmp");
            } else if (left->getType()->isFloatingPointTy()) {
                return builder->CreateFCmpOLT(left, right, "lttmp");
            }
        } else if (op == ">") {
            if (left->getType()->isIntegerTy()) {
                return builder->CreateICmpSGT(left, right, "gttmp");
            } else if (left->getType()->isFloatingPointTy()) {
                return builder->CreateFCmpOGT(left, right, "gttmp");
            }
        } else if (op == "<=") {
            if (left->getType()->isIntegerTy()) {
                return builder->CreateICmpSLE(left, right, "letmp");
            } else if (left->getType()->isFloatingPointTy()) {
                return builder->CreateFCmpOLE(left, right, "letmp");
            }
        } else if (op == ">=") {
            if (left->getType()->isIntegerTy()) {
                return builder->CreateICmpSGE(left, right, "getmp");
            } else if (left->getType()->isFloatingPointTy()) {
                return builder->CreateFCmpOGE(left, right, "getmp");
            }
        } else if (op == "&&") {
            return builder->CreateAnd(left, right, "andtmp");
        } else if (op == "||") {
            return builder->CreateOr(left, right, "ortmp");
        }
        
        return nullptr;
    }
    
    // 创建函数调用
    llvm::CallInst* createCall(const std::string& funcName, 
                              const std::vector<llvm::Value*>& args) {
        auto it = functions.find(funcName);
        if (it != functions.end()) {
            return builder->CreateCall(it->second, args);
        }
        return nullptr;
    }
    
    // 创建返回指令
    llvm::ReturnInst* createReturn(llvm::Value* value = nullptr) {
        if (value) {
            return builder->CreateRet(value);
        } else {
            return builder->CreateRetVoid();
        }
    }
    
    // 创建条件分支
    llvm::BranchInst* createCondBr(llvm::Value* cond, 
                                  llvm::BasicBlock* trueBB, 
                                  llvm::BasicBlock* falseBB) {
        return builder->CreateCondBr(cond, trueBB, falseBB);
    }
    
    // 创建无条件分支
    llvm::BranchInst* createBr(llvm::BasicBlock* destBB) {
        return builder->CreateBr(destBB);
    }
    
    // 获取变量值
    llvm::Value* getVariable(const std::string& name) {
        auto it = namedValues.find(name);
        if (it != namedValues.end()) {
            return it->second;
        }
        return nullptr;
    }
    
    // 获取模块
    llvm::Module* getModule() {
        return module.get();
    }
    
    // 获取上下文
    llvm::LLVMContext& getContext() {
        return *context;
    }
    
    // 获取构建器
    llvm::IRBuilder<>& getBuilder() {
        return *builder;
    }
    
    // 验证模块
    bool verifyModule() {
        return !llvm::verifyModule(*module, &llvm::errs());
    }
    
    // 打印模块
    void printModule() {
        module->print(llvm::outs(), nullptr);
    }
};

// 全局IR构建器实例
static std::unique_ptr<IRBuilder> globalIRBuilder;

// 初始化IR构建器
void initializeIRBuilder(const std::string& moduleName) {
    globalIRBuilder = std::make_unique<IRBuilder>(moduleName);
}

// 获取全局IR构建器
IRBuilder* getIRBuilder() {
    return globalIRBuilder.get();
}

// 清理IR构建器
void cleanupIRBuilder() {
    globalIRBuilder.reset();
}

} // namespace codegen
} // namespace starry