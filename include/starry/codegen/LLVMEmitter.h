#pragma once

#include <llvm/IR/LLVMContext.h>
#include <llvm/IR/Module.h>
#include <llvm/IR/IRBuilder.h>
#include <llvm/IR/Value.h>
#include <llvm/IR/Type.h>
#include <memory>
#include <unordered_map>
#include <string>

namespace starry {
namespace ast {
    class Program;
    class Expression;
    class Statement;
    class Function;
}

namespace starry {
namespace codegen {

/**
 * @brief LLVM代码发射器类
 * 
 * 负责将AST转换为LLVM IR代码
 */
class LLVMEmitter {
public:
    LLVMEmitter();
    ~LLVMEmitter();

    /**
     * @brief 为程序生成LLVM IR代码
     * @param program 程序AST根节点
     * @return 生成的LLVM模块
     */
    llvm::Module* emitProgram(ast::Program* program);

    /**
     * @brief 为表达式生成LLVM IR代码
     * @param expr 表达式节点
     * @return 生成的LLVM值
     */
    llvm::Value* emitExpression(ast::Expression* expr);

    /**
     * @brief 为语句生成LLVM IR代码
     * @param stmt 语句节点
     */
    void emitStatement(ast::Statement* stmt);

    /**
     * @brief 获取LLVM上下文
     * @return LLVM上下文引用
     */
    llvm::LLVMContext& getContext() { return *context_; }

    /**
     * @brief 获取LLVM模块
     * @return LLVM模块指针
     */
    llvm::Module* getModule() { return module_.get(); }

    /**
     * @brief 获取IR构建器
     * @return IR构建器引用
     */
    llvm::IRBuilder<>& getBuilder() { return *builder_; }

    /**
     * @brief 将模块输出为字符串
     * @return LLVM IR代码字符串
     */
    std::string getModuleString();

    /**
     * @brief 优化生成的代码
     */
    void optimize();

    /**
     * @brief 验证生成的模块
     * @return 验证成功返回true，否则返回false
     */
    bool verify();

private:
    /**
     * @brief 初始化类型映射
     */
    void initializeTypes();

    /**
     * @brief 获取LLVM类型
     * @param typeName 类型名称
     * @return 对应的LLVM类型
     */
    llvm::Type* getLLVMType(const std::string& typeName);

    /**
     * @brief 创建函数
     * @param name 函数名
     * @param returnType 返回类型
     * @param paramTypes 参数类型列表
     * @return 创建的函数
     */
    llvm::Function* createFunction(const std::string& name, 
                                  llvm::Type* returnType,
                                  const std::vector<llvm::Type*>& paramTypes);

    /**
     * @brief 创建基本块
     * @param name 基本块名称
     * @param function 所属函数
     * @return 创建的基本块
     */
    llvm::BasicBlock* createBasicBlock(const std::string& name, 
                                      llvm::Function* function);

private:
    std::unique_ptr<llvm::LLVMContext> context_;           ///< LLVM上下文
    std::unique_ptr<llvm::Module> module_;                 ///< LLVM模块
    std::unique_ptr<llvm::IRBuilder<>> builder_;           ///< IR构建器
    
    std::unordered_map<std::string, llvm::Type*> typeMap_; ///< 类型映射表
    std::unordered_map<std::string, llvm::Value*> valueMap_; ///< 值映射表
    std::unordered_map<std::string, llvm::Function*> functionMap_; ///< 函数映射表
};

} // namespace codegen
} // namespace starry