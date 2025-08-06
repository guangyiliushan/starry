#include "starry/semantic/Error.h"
#include <sstream>
#include <algorithm>

namespace starry {
namespace semantic {

// SemanticError 实现
SemanticError::SemanticError(ErrorType type, const std::string& message, 
                           const SourceLocation& location)
    : type_(type), message_(message), location_(location) {}

SemanticError::ErrorType SemanticError::getType() const {
    return type_;
}

const std::string& SemanticError::getMessage() const {
    return message_;
}

const SourceLocation& SemanticError::getLocation() const {
    return location_;
}

std::string SemanticError::toString() const {
    std::ostringstream oss;
    oss << getErrorTypeName(type_) << " at " 
        << location_.filename << ":" << location_.line << ":" << location_.column
        << " - " << message_;
    return oss.str();
}

std::string SemanticError::getErrorTypeName(ErrorType type) {
    switch (type) {
        case ErrorType::TYPE_MISMATCH:
            return "类型不匹配错误";
        case ErrorType::UNDEFINED_SYMBOL:
            return "未定义符号错误";
        case ErrorType::REDEFINITION:
            return "重定义错误";
        case ErrorType::INVALID_OPERATION:
            return "无效操作错误";
        case ErrorType::SCOPE_ERROR:
            return "作用域错误";
        case ErrorType::ACCESS_VIOLATION:
            return "访问权限错误";
        case ErrorType::CIRCULAR_DEPENDENCY:
            return "循环依赖错误";
        case ErrorType::INVALID_CAST:
            return "无效类型转换错误";
        case ErrorType::MISSING_RETURN:
            return "缺少返回值错误";
        case ErrorType::UNREACHABLE_CODE:
            return "不可达代码错误";
        default:
            return "未知错误";
    }
}

// ErrorReporter 实现
ErrorReporter::ErrorReporter() : maxErrors_(100), errorCount_(0), warningCount_(0) {}

void ErrorReporter::reportError(SemanticError::ErrorType type, 
                               const std::string& message,
                               const SourceLocation& location) {
    if (errorCount_ >= maxErrors_) {
        return; // 达到最大错误数量限制
    }
    
    SemanticError error(type, message, location);
    errors_.push_back(error);
    errorCount_++;
    
    // 输出错误信息
    std::cerr << "错误: " << error.toString() << std::endl;
}

void ErrorReporter::reportWarning(const std::string& message,
                                 const SourceLocation& location) {
    Warning warning{message, location};
    warnings_.push_back(warning);
    warningCount_++;
    
    // 输出警告信息
    std::cerr << "警告: " << message << " at " 
              << location.filename << ":" << location.line << ":" << location.column
              << std::endl;
}

void ErrorReporter::reportTypeMismatch(const std::string& expected,
                                      const std::string& actual,
                                      const SourceLocation& location) {
    std::ostringstream oss;
    oss << "期望类型 '" << expected << "'，但得到 '" << actual << "'";
    reportError(SemanticError::ErrorType::TYPE_MISMATCH, oss.str(), location);
}

void ErrorReporter::reportUndefinedSymbol(const std::string& symbol,
                                         const SourceLocation& location) {
    std::ostringstream oss;
    oss << "未定义的符号 '" << symbol << "'";
    reportError(SemanticError::ErrorType::UNDEFINED_SYMBOL, oss.str(), location);
}

void ErrorReporter::reportRedefinition(const std::string& symbol,
                                      const SourceLocation& location,
                                      const SourceLocation& previousLocation) {
    std::ostringstream oss;
    oss << "符号 '" << symbol << "' 重定义，之前定义在 "
        << previousLocation.filename << ":" << previousLocation.line;
    reportError(SemanticError::ErrorType::REDEFINITION, oss.str(), location);
}

void ErrorReporter::reportInvalidOperation(const std::string& operation,
                                          const std::string& type,
                                          const SourceLocation& location) {
    std::ostringstream oss;
    oss << "无效操作 '" << operation << "' 应用于类型 '" << type << "'";
    reportError(SemanticError::ErrorType::INVALID_OPERATION, oss.str(), location);
}

void ErrorReporter::reportScopeError(const std::string& message,
                                    const SourceLocation& location) {
    reportError(SemanticError::ErrorType::SCOPE_ERROR, message, location);
}

void ErrorReporter::reportAccessViolation(const std::string& symbol,
                                         const std::string& accessLevel,
                                         const SourceLocation& location) {
    std::ostringstream oss;
    oss << "无法访问 " << accessLevel << " 成员 '" << symbol << "'";
    reportError(SemanticError::ErrorType::ACCESS_VIOLATION, oss.str(), location);
}

void ErrorReporter::reportCircularDependency(const std::string& symbol,
                                            const SourceLocation& location) {
    std::ostringstream oss;
    oss << "检测到循环依赖，涉及符号 '" << symbol << "'";
    reportError(SemanticError::ErrorType::CIRCULAR_DEPENDENCY, oss.str(), location);
}

void ErrorReporter::reportInvalidCast(const std::string& fromType,
                                     const std::string& toType,
                                     const SourceLocation& location) {
    std::ostringstream oss;
    oss << "无法将类型 '" << fromType << "' 转换为 '" << toType << "'";
    reportError(SemanticError::ErrorType::INVALID_CAST, oss.str(), location);
}

void ErrorReporter::reportMissingReturn(const std::string& function,
                                       const SourceLocation& location) {
    std::ostringstream oss;
    oss << "函数 '" << function << "' 缺少返回语句";
    reportError(SemanticError::ErrorType::MISSING_RETURN, oss.str(), location);
}

void ErrorReporter::reportUnreachableCode(const SourceLocation& location) {
    reportError(SemanticError::ErrorType::UNREACHABLE_CODE, 
               "检测到不可达代码", location);
}

bool ErrorReporter::hasErrors() const {
    return errorCount_ > 0;
}

bool ErrorReporter::hasWarnings() const {
    return warningCount_ > 0;
}

size_t ErrorReporter::getErrorCount() const {
    return errorCount_;
}

size_t ErrorReporter::getWarningCount() const {
    return warningCount_;
}

const std::vector<SemanticError>& ErrorReporter::getErrors() const {
    return errors_;
}

const std::vector<ErrorReporter::Warning>& ErrorReporter::getWarnings() const {
    return warnings_;
}

void ErrorReporter::clear() {
    errors_.clear();
    warnings_.clear();
    errorCount_ = 0;
    warningCount_ = 0;
}

void ErrorReporter::setMaxErrors(size_t maxErrors) {
    maxErrors_ = maxErrors;
}

void ErrorReporter::printSummary() const {
    if (errorCount_ > 0) {
        std::cerr << "\n编译失败: " << errorCount_ << " 个错误";
        if (warningCount_ > 0) {
            std::cerr << ", " << warningCount_ << " 个警告";
        }
        std::cerr << std::endl;
    } else if (warningCount_ > 0) {
        std::cerr << "\n编译成功: " << warningCount_ << " 个警告" << std::endl;
    } else {
        std::cerr << "\n编译成功: 无错误或警告" << std::endl;
    }
}

// ErrorRecovery 实现
ErrorRecovery::ErrorRecovery() {}

bool ErrorRecovery::canRecover(SemanticError::ErrorType errorType) const {
    switch (errorType) {
        case SemanticError::ErrorType::TYPE_MISMATCH:
        case SemanticError::ErrorType::UNDEFINED_SYMBOL:
        case SemanticError::ErrorType::INVALID_OPERATION:
            return true; // 这些错误可以尝试恢复
        case SemanticError::ErrorType::REDEFINITION:
        case SemanticError::ErrorType::CIRCULAR_DEPENDENCY:
            return false; // 这些错误难以恢复
        default:
            return false;
    }
}

void ErrorRecovery::suggestFix(const SemanticError& error, 
                              std::vector<std::string>& suggestions) const {
    suggestions.clear();
    
    switch (error.getType()) {
        case SemanticError::ErrorType::TYPE_MISMATCH:
            suggestions.push_back("检查变量类型是否正确");
            suggestions.push_back("考虑添加类型转换");
            break;
            
        case SemanticError::ErrorType::UNDEFINED_SYMBOL:
            suggestions.push_back("检查符号名称拼写");
            suggestions.push_back("确保符号已声明");
            suggestions.push_back("检查作用域是否正确");
            break;
            
        case SemanticError::ErrorType::REDEFINITION:
            suggestions.push_back("使用不同的符号名称");
            suggestions.push_back("检查是否有重复的声明");
            break;
            
        case SemanticError::ErrorType::INVALID_OPERATION:
            suggestions.push_back("检查操作符是否适用于该类型");
            suggestions.push_back("考虑重载操作符");
            break;
            
        case SemanticError::ErrorType::MISSING_RETURN:
            suggestions.push_back("在函数末尾添加return语句");
            suggestions.push_back("确保所有代码路径都有返回值");
            break;
            
        default:
            suggestions.push_back("请检查代码逻辑");
            break;
    }
}

std::string ErrorRecovery::generateFixHint(const SemanticError& error) const {
    std::vector<std::string> suggestions;
    suggestFix(error, suggestions);
    
    if (suggestions.empty()) {
        return "无可用修复建议";
    }
    
    std::ostringstream oss;
    oss << "修复建议:\n";
    for (size_t i = 0; i < suggestions.size(); ++i) {
        oss << "  " << (i + 1) << ". " << suggestions[i] << "\n";
    }
    
    return oss.str();
}

} // namespace semantic
} // namespace starry