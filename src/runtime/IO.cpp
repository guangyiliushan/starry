#include "starry/runtime/IO.h"
#include <iostream>
#include <fstream>
#include <sstream>

namespace starry {
namespace runtime {

// IOManager 类实现
IOManager::IOManager() : initialized_(false) {}

IOManager::~IOManager() {
    cleanup();
}

bool IOManager::initialize() {
    if (initialized_) return true;
    
    // 初始化标准流
    standardStreams_[StreamType::STDIN] = &std::cin;
    standardStreams_[StreamType::STDOUT] = &std::cout;
    standardStreams_[StreamType::STDERR] = &std::cerr;
    
    initialized_ = true;
    return true;
}

void IOManager::cleanup() {
    if (!initialized_) return;
    
    // 关闭所有打开的文件流
    for (auto& pair : fileStreams_) {
        if (pair.second && pair.second->is_open()) {
            pair.second->close();
        }
        delete pair.second;
    }
    fileStreams_.clear();
    
    standardStreams_.clear();
    initialized_ = false;
}

StreamHandle IOManager::openFile(const std::string& filename, IOMode mode) {
    if (!initialized_) {
        initialize();
    }
    
    std::ios::openmode openMode = std::ios::in;
    switch (mode) {
        case IOMode::READ:
            openMode = std::ios::in;
            break;
        case IOMode::write:
            openMode = std::ios::out;
            break;
        case IOMode::append:
            openMode = std::ios::out | std::ios::app;
            break;
        case IOMode::read_write:
            openMode = std::ios::in | std::ios::out;
            break;
        case IOMode::binary_read:
            openMode = std::ios::in | std::ios::binary;
            break;
        case IOMode::binary_write:
            openMode = std::ios::out | std::ios::binary;
            break;
    }
    
    auto* fileStream = new std::fstream(filename, openMode);
    if (!fileStream->is_open()) {
        delete fileStream;
        return INVALID_HANDLE;
    }
    
    StreamHandle handle = nextHandle_++;
    fileStreams_[handle] = fileStream;
    return handle;
}

bool IOManager::closeFile(StreamHandle handle) {
    auto it = fileStreams_.find(handle);
    if (it == fileStreams_.end()) {
        return false;
    }
    
    if (it->second && it->second->is_open()) {
        it->second->close();
    }
    delete it->second;
    fileStreams_.erase(it);
    
    return true;
}

std::string IOManager::readString(StreamHandle handle, size_t maxLength) {
    std::istream* stream = getInputStream(handle);
    if (!stream) return "";
    
    std::string result;
    if (maxLength == 0) {
        // 读取整个流
        std::ostringstream buffer;
        buffer << stream->rdbuf();
        result = buffer.str();
    } else {
        // 读取指定长度
        result.resize(maxLength);
        stream->read(&result[0], maxLength);
        result.resize(stream->gcount());
    }
    
    return result;
}

std::string IOManager::readLine(StreamHandle handle) {
    std::istream* stream = getInputStream(handle);
    if (!stream) return "";
    
    std::string line;
    std::getline(*stream, line);
    return line;
}

std::vector<uint8_t> IOManager::readBytes(StreamHandle handle, size_t count) {
    std::istream* stream = getInputStream(handle);
    if (!stream) return {};
    
    std::vector<uint8_t> buffer(count);
    stream->read(reinterpret_cast<char*>(buffer.data()), count);
    buffer.resize(stream->gcount());
    
    return buffer;
}

bool IOManager::writeString(StreamHandle handle, const std::string& data) {
    std::ostream* stream = getOutputStream(handle);
    if (!stream) return false;
    
    *stream << data;
    return stream->good();
}

bool IOManager::writeLine(StreamHandle handle, const std::string& line) {
    std::ostream* stream = getOutputStream(handle);
    if (!stream) return false;
    
    *stream << line << std::endl;
    return stream->good();
}

bool IOManager::writeBytes(StreamHandle handle, const std::vector<uint8_t>& data) {
    std::ostream* stream = getOutputStream(handle);
    if (!stream) return false;
    
    stream->write(reinterpret_cast<const char*>(data.data()), data.size());
    return stream->good();
}

bool IOManager::flush(StreamHandle handle) {
    std::ostream* stream = getOutputStream(handle);
    if (!stream) return false;
    
    stream->flush();
    return true;
}

bool IOManager::isEOF(StreamHandle handle) {
    std::istream* stream = getInputStream(handle);
    if (!stream) return true;
    
    return stream->eof();
}

bool IOManager::isGood(StreamHandle handle) {
    if (handle < STANDARD_STREAM_COUNT) {
        auto it = standardStreams_.find(static_cast<StreamType>(handle));
        if (it != standardStreams_.end()) {
            return it->second->good();
        }
    }
    
    auto it = fileStreams_.find(handle);
    if (it != fileStreams_.end() && it->second) {
        return it->second->good();
    }
    
    return false;
}

size_t IOManager::getPosition(StreamHandle handle) {
    std::istream* stream = getInputStream(handle);
    if (!stream) return 0;
    
    return stream->tellg();
}

bool IOManager::setPosition(StreamHandle handle, size_t position) {
    auto it = fileStreams_.find(handle);
    if (it == fileStreams_.end() || !it->second) {
        return false;
    }
    
    it->second->seekg(position);
    it->second->seekp(position);
    return it->second->good();
}

std::istream* IOManager::getInputStream(StreamHandle handle) {
    if (handle < STANDARD_STREAM_COUNT) {
        auto it = standardStreams_.find(static_cast<StreamType>(handle));
        if (it != standardStreams_.end()) {
            return dynamic_cast<std::istream*>(it->second);
        }
    }
    
    auto it = fileStreams_.find(handle);
    if (it != fileStreams_.end()) {
        return it->second;
    }
    
    return nullptr;
}

std::ostream* IOManager::getOutputStream(StreamHandle handle) {
    if (handle < STANDARD_STREAM_COUNT) {
        auto it = standardStreams_.find(static_cast<StreamType>(handle));
        if (it != standardStreams_.end()) {
            return dynamic_cast<std::ostream*>(it->second);
        }
    }
    
    auto it = fileStreams_.find(handle);
    if (it != fileStreams_.end()) {
        return it->second;
    }
    
    return nullptr;
}

// 全局IO管理器实例
static IOManager g_ioManager;

// 全局函数实现
bool initializeIO() {
    return g_ioManager.initialize();
}

void cleanupIO() {
    g_ioManager.cleanup();
}

StreamHandle openFile(const std::string& filename, IOMode mode) {
    return g_ioManager.openFile(filename, mode);
}

bool closeFile(StreamHandle handle) {
    return g_ioManager.closeFile(handle);
}

std::string readString(StreamHandle handle, size_t maxLength) {
    return g_ioManager.readString(handle, maxLength);
}

std::string readLine(StreamHandle handle) {
    return g_ioManager.readLine(handle);
}

std::vector<uint8_t> readBytes(StreamHandle handle, size_t count) {
    return g_ioManager.readBytes(handle, count);
}

bool writeString(StreamHandle handle, const std::string& data) {
    return g_ioManager.writeString(handle, data);
}

bool writeLine(StreamHandle handle, const std::string& line) {
    return g_ioManager.writeLine(handle, line);
}

bool writeBytes(StreamHandle handle, const std::vector<uint8_t>& data) {
    return g_ioManager.writeBytes(handle, data);
}

bool flush(StreamHandle handle) {
    return g_ioManager.flush(handle);
}

bool isEOF(StreamHandle handle) {
    return g_ioManager.isEOF(handle);
}

bool isGood(StreamHandle handle) {
    return g_ioManager.isGood(handle);
}

size_t getPosition(StreamHandle handle) {
    return g_ioManager.getPosition(handle);
}

bool setPosition(StreamHandle handle, size_t position) {
    return g_ioManager.setPosition(handle, position);
}

// 便利函数
void print(const std::string& message) {
    writeString(STDOUT_HANDLE, message);
}

void println(const std::string& message) {
    writeLine(STDOUT_HANDLE, message);
}

void printError(const std::string& message) {
    writeString(STDERR_HANDLE, message);
}

void printErrorLine(const std::string& message) {
    writeLine(STDERR_HANDLE, message);
}

std::string input() {
    return readLine(STDIN_HANDLE);
}

std::string input(const std::string& prompt) {
    print(prompt);
    return input();
}

} // namespace runtime
} // namespace starry