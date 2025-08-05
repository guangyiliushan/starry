#include <iostream>
#include <string>
#include <fstream>
#include <memory>
#include <vector>

#include "compiler/lexer/Lexer.h"
#include "compiler/parser/Parser.h"
#include "compiler/ast/ASTNode.h"
#include "compiler/semantic/SemanticAnalyzer.h"
#include "compiler/codegen/CodeGenerator.h"

void printUsage() {
    std::cout << "Starry编程语言编译器 v0.1.0" << std::endl;
    std::cout << "用法: starry [选项] <输入文件>" << std::endl;
    std::cout << "选项:" << std::endl;
    std::cout << "  -o <文件>    指定输出文件" << std::endl;
    std::cout << "  -S           输出汇编代码" << std::endl;
    std::cout << "  -c           仅编译不链接" << std::endl;
    std::cout << "  -O<级别>     优化级别 (0-3)" << std::endl;
    std::cout << "  -v           显示版本信息" << std::endl;
    std::cout << "  -h           显示帮助信息" << std::endl;
}

int main(int argc, char* argv[]) {
    if (argc < 2) {
        printUsage();
        return 1;
    }

    std::string inputFile;
    std::string outputFile = "a.out";
    bool outputAssembly = false;
    bool compileOnly = false;
    int optimizationLevel = 0;

    // 解析命令行参数
    for (int i = 1; i < argc; ++i) {
        std::string arg = argv[i];
        if (arg[0] == '-') {
            switch (arg[1]) {
                case 'o':
                    if (i + 1 < argc) {
                        outputFile = argv[++i];
                    } else {
                        std::cerr << "错误: -o 选项需要一个参数" << std::endl;
                        return 1;
                    }
                    break;
                case 'S':
                    outputAssembly = true;
                    break;
                case 'c':
                    compileOnly = true;
                    break;
                case 'O':
                    if (arg.length() > 2) {
                        optimizationLevel = arg[2] - '0';
                        if (optimizationLevel < 0 || optimizationLevel > 3) {
                            std::cerr << "错误: 无效的优化级别" << std::endl;
                            return 1;
                        }
                    }
                    break;
                case 'v':
                    std::cout << "Starry编程语言编译器 v0.1.0" << std::endl;
                    return 0;
                case 'h':
                    printUsage();
                    return 0;
                default:
                    std::cerr << "错误: 未知选项 " << arg << std::endl;
                    return 1;
            }
        } else {
            inputFile = arg;
        }
    }

    if (inputFile.empty()) {
        std::cerr << "错误: 未指定输入文件" << std::endl;
        return 1;
    }

    try {
        // 读取输入文件
        std::ifstream file(inputFile);
        if (!file) {
            std::cerr << "错误: 无法打开文件 " << inputFile << std::endl;
            return 1;
        }

        std::string sourceCode((std::istreambuf_iterator<char>(file)),
                               std::istreambuf_iterator<char>());

        // 编译流程
        std::cout << "编译 " << inputFile << std::endl;

        // 1. 词法分析
        auto lexer = std::make_unique<starry::Lexer>(sourceCode);
        auto tokens = lexer->tokenize();
        
        // 2. 语法分析
        auto parser = std::make_unique<starry::Parser>(tokens);
        auto ast = parser->parse();
        
        // 3. 语义分析
        auto semanticAnalyzer = std::make_unique<starry::SemanticAnalyzer>();
        semanticAnalyzer->analyze(ast);
        
        // 4. 代码生成
        auto codeGenerator = std::make_unique<starry::CodeGenerator>(optimizationLevel);
        codeGenerator->generate(ast, outputFile, outputAssembly, compileOnly);
        
        std::cout << "编译成功: " << outputFile << std::endl;
        return 0;
    } catch (const std::exception& e) {
        std::cerr << "编译错误: " << e.what() << std::endl;
        return 1;
    }
}