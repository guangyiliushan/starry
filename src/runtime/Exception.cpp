#include "starry/runtime/Exception.h"
#include <iostream>
#include <sstream>
#include <cstring>

namespace starry {
namespace runtime {

// 基础异常类实现
StarryException::StarryException(const std::string& message, int code)
    : message_(message), errorCode_(code) {
    // 构建完整的错误信息
    std::ostringstream oss;
    oss << "Starry异常 [" << code << "]: " << message;
    fullMessage_ = oss.str();
}

const char* StarryException::what() const noexcept {
    return fullMessage_.c_str();
}

const std::string& StarryException::getMessage() const {
    return message_;
}

int StarryException::getErrorCode() const {
    return errorCode_;
}

// 运行时异常实现
RuntimeException::RuntimeException(const std::string& message, int code)
    : StarryException("运行时错误: " + message, code) {}

// 内存异常实现
MemoryException::MemoryException(const std::string& message, int code)
    : RuntimeException("内存错误: " + message, code) {}

// 类型异常实现
TypeException::TypeException(const std::string& message, int code)
    : RuntimeException("类型错误: " + message, code) {}

// 索引异常实现
IndexException::IndexException(const std::string& message, int code)
    : RuntimeException("索引错误: " + message, code) {}

// 空指针异常实现
NullPointerException::NullPointerException(const std::string& message, int code)
    : RuntimeException("空指针错误: " + message, code) {}

// 除零异常实现
DivisionByZeroException::DivisionByZeroException(const std::string& message, int code)
    : RuntimeException("除零错误: " + message, code) {}

// 文件异常实现
FileException::FileException(const std::string& message, int code)
    : RuntimeException("文件错误: " + message, code) {}

// 网络异常实现
NetworkException::NetworkException(const std::string& message, int code)
    : RuntimeException("网络错误: " + message, code) {}

// 线程异常实现
ThreadException::ThreadException(const std::string& message, int code)
    : RuntimeException("线程错误: " + message, code) {}

// 异常处理器实现
class ExceptionHandler {
private:
    static ExceptionHandler* instance_;
    std::vector<std::function<void(const StarryException&)>> handlers_;
    bool enableLogging_;
    
    ExceptionHandler() : enableLogging_(true) {}
    
public:
    static ExceptionHandler& getInstance() {
        if (!instance_) {
            instance_ = new ExceptionHandler();
        }
        return *instance_;
    }
    
    // 注册异常处理器
    void registerHandler(std::function<void(const StarryException&)> handler) {
        handlers_.push_back(handler);
    }
    
    // 处理异常
    void handleException(const StarryException& ex) {
        // 记录异常日志
        if (enableLogging_) {
            logException(ex);
        }
        
        // 调用注册的处理器
        for (auto& handler : handlers_) {
            try {
                handler(ex);
            } catch (...) {
                // 处理器本身出现异常，记录但不抛出
                std::cerr << "异常处理器执行失败" << std::endl;
            }
        }
    }
    
    // 记录异常日志
    void logException(const StarryException& ex) {
        std::cerr << "[异常日志] " << ex.what() << std::endl;
        std::cerr << "错误代码: " << ex.getErrorCode() << std::endl;
        std::cerr << "时间戳: " << getCurrentTimestamp() << std::endl;
        std::cerr << "---" << std::endl;
    }
    
    // 启用/禁用日志记录
    void setLoggingEnabled(bool enabled) {
        enableLogging_ = enabled;
    }
    
    // 清除所有处理器
    void clearHandlers() {
        handlers_.clear();
    }
    
private:
    std::string getCurrentTimestamp() {
        auto now = std::chrono::system_clock::now();
        auto time_t = std::chrono::system_clock::to_time_t(now);
        std::ostringstream oss;
        oss << std::put_time(std::localtime(&time_t), "%Y-%m-%d %H:%M:%S");
        return oss.str();
    }
};

// 静态成员初始化
ExceptionHandler* ExceptionHandler::instance_ = nullptr;

// 全局异常处理函数
void handleException(const StarryException& ex) {
    ExceptionHandler::getInstance().handleException(ex);
}

void registerExceptionHandler(std::function<void(const StarryException&)> handler) {
    ExceptionHandler::getInstance().registerHandler(handler);
}

void setExceptionLogging(bool enabled) {
    ExceptionHandler::getInstance().setLoggingEnabled(enabled);
}

// 异常工厂函数
std::unique_ptr<StarryException> createException(ExceptionType type, 
                                               const std::string& message, 
                                               int code) {
    switch (type) {
        case ExceptionType::RUNTIME:
            return std::make_unique<RuntimeException>(message, code);
        case ExceptionType::MEMORY:
            return std::make_unique<MemoryException>(message, code);
        case ExceptionType::TYPE:
            return std::make_unique<TypeException>(message, code);
        case ExceptionType::INDEX:
            return std::make_unique<IndexException>(message, code);
        case ExceptionType::NULL_POINTER:
            return std::make_unique<NullPointerException>(message, code);
        case ExceptionType::DIVISION_BY_ZERO:
            return std::make_unique<DivisionByZeroException>(message, code);
        case ExceptionType::FILE:
            return std::make_unique<FileException>(message, code);
        case ExceptionType::NETWORK:
            return std::make_unique<NetworkException>(message, code);
        case ExceptionType::THREAD:
            return std::make_unique<ThreadException>(message, code);
        default:
            return std::make_unique<StarryException>(message, code);
    }
}

// 便捷的抛出异常宏实现
void throwRuntimeError(const std::string& message, int code) {
    throw RuntimeException(message, code);
}

void throwMemoryError(const std::string& message, int code) {
    throw MemoryException(message, code);
}

void throwTypeError(const std::string& message, int code) {
    throw TypeException(message, code);
}

void throwIndexError(const std::string& message, int code) {
    throw IndexException(message, code);
}

void throwNullPointerError(const std::string& message, int code) {
    throw NullPointerException(message, code);
}

void throwDivisionByZeroError(const std::string& message, int code) {
    throw DivisionByZeroException(message, code);
}

void throwFileError(const std::string& message, int code) {
    throw FileException(message, code);
}

void throwNetworkError(const std::string& message, int code) {
    throw NetworkException(message, code);
}

void throwThreadError(const std::string& message, int code) {
    throw ThreadException(message, code);
}

// 异常安全的资源管理
template<typename T>
class ExceptionSafeWrapper {
private:
    T* resource_;
    std::function<void(T*)> deleter_;
    
public:
    ExceptionSafeWrapper(T* resource, std::function<void(T*)> deleter)
        : resource_(resource), deleter_(deleter) {}
    
    ~ExceptionSafeWrapper() {
        if (resource_ && deleter_) {
            try {
                deleter_(resource_);
            } catch (...) {
                // 析构函数中不抛出异常
            }
        }
    }
    
    T* get() const { return resource_; }
    T* release() {
        T* temp = resource_;
        resource_ = nullptr;
        return temp;
    }
    
    // 禁止拷贝
    ExceptionSafeWrapper(const ExceptionSafeWrapper&) = delete;
    ExceptionSafeWrapper& operator=(const ExceptionSafeWrapper&) = delete;
    
    // 允许移动
    ExceptionSafeWrapper(ExceptionSafeWrapper&& other) noexcept
        : resource_(other.resource_), deleter_(std::move(other.deleter_)) {
        other.resource_ = nullptr;
    }
    
    ExceptionSafeWrapper& operator=(ExceptionSafeWrapper&& other) noexcept {
        if (this != &other) {
            if (resource_ && deleter_) {
                try {
                    deleter_(resource_);
                } catch (...) {}
            }
            resource_ = other.resource_;
            deleter_ = std::move(other.deleter_);
            other.resource_ = nullptr;
        }
        return *this;
    }
};

// 异常安全的函数执行
template<typename Func>
auto executeWithExceptionHandling(Func&& func) -> decltype(func()) {
    try {
        return func();
    } catch (const StarryException& ex) {
        handleException(ex);
        throw;
    } catch (const std::exception& ex) {
        StarryException starryEx("标准异常: " + std::string(ex.what()), -1);
        handleException(starryEx);
        throw starryEx;
    } catch (...) {
        StarryException starryEx("未知异常", -2);
        handleException(starryEx);
        throw starryEx;
    }
}

} // namespace runtime
} // namespace starry