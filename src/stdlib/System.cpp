#include "starry/stdlib/System.h"
#include <chrono>
#include <thread>
#include <cstdlib>
#include <iostream>
#include <sstream>

#ifdef _WIN32
#include <windows.h>
#include <psapi.h>
#else
#include <unistd.h>
#include <sys/resource.h>
#include <sys/utsname.h>
#endif

namespace starry {
namespace stdlib {

// System 类实现
System::System() {}

std::string System::getOperatingSystem() {
#ifdef _WIN32
    return "Windows";
#elif defined(__APPLE__)
    return "macOS";
#elif defined(__linux__)
    return "Linux";
#elif defined(__unix__)
    return "Unix";
#else
    return "Unknown";
#endif
}

std::string System::getArchitecture() {
#ifdef _WIN64
    return "x64";
#elif defined(_WIN32)
    return "x86";
#elif defined(__x86_64__)
    return "x64";
#elif defined(__i386__)
    return "x86";
#elif defined(__aarch64__)
    return "ARM64";
#elif defined(__arm__)
    return "ARM";
#else
    return "Unknown";
#endif
}

std::string System::getVersion() {
#ifdef _WIN32
    OSVERSIONINFO osvi;
    ZeroMemory(&osvi, sizeof(OSVERSIONINFO));
    osvi.dwOSVersionInfoSize = sizeof(OSVERSIONINFO);
    
    if (GetVersionEx(&osvi)) {
        std::ostringstream oss;
        oss << osvi.dwMajorVersion << "." << osvi.dwMinorVersion;
        return oss.str();
    }
    return "Unknown";
#else
    struct utsname unameData;
    if (uname(&unameData) == 0) {
        return std::string(unameData.release);
    }
    return "Unknown";
#endif
}

size_t System::getProcessorCount() {
#ifdef _WIN32
    SYSTEM_INFO sysInfo;
    GetSystemInfo(&sysInfo);
    return sysInfo.dwNumberOfProcessors;
#else
    return sysconf(_SC_NPROCESSORS_ONLN);
#endif
}

size_t System::getTotalMemory() {
#ifdef _WIN32
    MEMORYSTATUSEX memInfo;
    memInfo.dwLength = sizeof(MEMORYSTATUSEX);
    if (GlobalMemoryStatusEx(&memInfo)) {
        return memInfo.ullTotalPhys;
    }
    return 0;
#else
    long pages = sysconf(_SC_PHYS_PAGES);
    long page_size = sysconf(_SC_PAGE_SIZE);
    return pages * page_size;
#endif
}

size_t System::getAvailableMemory() {
#ifdef _WIN32
    MEMORYSTATUSEX memInfo;
    memInfo.dwLength = sizeof(MEMORYSTATUSEX);
    if (GlobalMemoryStatusEx(&memInfo)) {
        return memInfo.ullAvailPhys;
    }
    return 0;
#else
    long pages = sysconf(_SC_AVPHYS_PAGES);
    long page_size = sysconf(_SC_PAGE_SIZE);
    return pages * page_size;
#endif
}

size_t System::getUsedMemory() {
    return getTotalMemory() - getAvailableMemory();
}

double System::getMemoryUsagePercentage() {
    size_t total = getTotalMemory();
    if (total == 0) return 0.0;
    
    size_t used = getUsedMemory();
    return (double)used / total * 100.0;
}

size_t System::getCurrentProcessMemory() {
#ifdef _WIN32
    PROCESS_MEMORY_COUNTERS pmc;
    if (GetProcessMemoryInfo(GetCurrentProcess(), &pmc, sizeof(pmc))) {
        return pmc.WorkingSetSize;
    }
    return 0;
#else
    struct rusage usage;
    if (getrusage(RUSAGE_SELF, &usage) == 0) {
        return usage.ru_maxrss * 1024; // Linux返回KB，转换为字节
    }
    return 0;
#endif
}

int System::getCurrentProcessId() {
#ifdef _WIN32
    return GetCurrentProcessId();
#else
    return getpid();
#endif
}

std::string System::getCurrentUser() {
#ifdef _WIN32
    char username[256];
    DWORD size = sizeof(username);
    if (GetUserNameA(username, &size)) {
        return std::string(username);
    }
    return "Unknown";
#else
    const char* user = getenv("USER");
    if (user) {
        return std::string(user);
    }
    return "Unknown";
#endif
}

std::string System::getEnvironmentVariable(const std::string& name) {
    const char* value = std::getenv(name.c_str());
    return value ? std::string(value) : "";
}

bool System::setEnvironmentVariable(const std::string& name, const std::string& value) {
#ifdef _WIN32
    return SetEnvironmentVariableA(name.c_str(), value.c_str()) != 0;
#else
    return setenv(name.c_str(), value.c_str(), 1) == 0;
#endif
}

std::map<std::string, std::string> System::getAllEnvironmentVariables() {
    std::map<std::string, std::string> envVars;
    
#ifdef _WIN32
    LPCH envStrings = GetEnvironmentStrings();
    if (envStrings) {
        LPCH current = envStrings;
        while (*current) {
            std::string envVar(current);
            size_t pos = envVar.find('=');
            if (pos != std::string::npos) {
                std::string name = envVar.substr(0, pos);
                std::string value = envVar.substr(pos + 1);
                envVars[name] = value;
            }
            current += envVar.length() + 1;
        }
        FreeEnvironmentStrings(envStrings);
    }
#else
    extern char** environ;
    for (char** env = environ; *env; ++env) {
        std::string envVar(*env);
        size_t pos = envVar.find('=');
        if (pos != std::string::npos) {
            std::string name = envVar.substr(0, pos);
            std::string value = envVar.substr(pos + 1);
            envVars[name] = value;
        }
    }
#endif
    
    return envVars;
}

int System::executeCommand(const std::string& command) {
    return std::system(command.c_str());
}

std::string System::executeCommandWithOutput(const std::string& command) {
    std::string result;
    
#ifdef _WIN32
    FILE* pipe = _popen(command.c_str(), "r");
#else
    FILE* pipe = popen(command.c_str(), "r");
#endif
    
    if (pipe) {
        char buffer[128];
        while (fgets(buffer, sizeof(buffer), pipe) != nullptr) {
            result += buffer;
        }
        
#ifdef _WIN32
        _pclose(pipe);
#else
        pclose(pipe);
#endif
    }
    
    return result;
}

void System::sleep(int milliseconds) {
    std::this_thread::sleep_for(std::chrono::milliseconds(milliseconds));
}

void System::exit(int exitCode) {
    std::exit(exitCode);
}

void System::abort() {
    std::abort();
}

// Time 类实现
Time::Time() {}

int64_t Time::getCurrentTimestamp() {
    auto now = std::chrono::system_clock::now();
    auto duration = now.time_since_epoch();
    return std::chrono::duration_cast<std::chrono::milliseconds>(duration).count();
}

int64_t Time::getCurrentTimestampSeconds() {
    auto now = std::chrono::system_clock::now();
    auto duration = now.time_since_epoch();
    return std::chrono::duration_cast<std::chrono::seconds>(duration).count();
}

int64_t Time::getCurrentTimestampMicroseconds() {
    auto now = std::chrono::system_clock::now();
    auto duration = now.time_since_epoch();
    return std::chrono::duration_cast<std::chrono::microseconds>(duration).count();
}

std::string Time::getCurrentTimeString() {
    auto now = std::chrono::system_clock::now();
    auto time_t = std::chrono::system_clock::to_time_t(now);
    
    std::ostringstream oss;
    oss << std::put_time(std::localtime(&time_t), "%Y-%m-%d %H:%M:%S");
    return oss.str();
}

std::string Time::formatTimestamp(int64_t timestamp, const std::string& format) {
    auto time_t = static_cast<std::time_t>(timestamp / 1000);
    
    std::ostringstream oss;
    oss << std::put_time(std::localtime(&time_t), format.c_str());
    return oss.str();
}

int64_t Time::parseTimeString(const std::string& timeStr, const std::string& format) {
    std::tm tm = {};
    std::istringstream ss(timeStr);
    ss >> std::get_time(&tm, format.c_str());
    
    if (ss.fail()) {
        return 0;
    }
    
    auto time_t = std::mktime(&tm);
    return static_cast<int64_t>(time_t) * 1000;
}

void Time::sleep(int milliseconds) {
    std::this_thread::sleep_for(std::chrono::milliseconds(milliseconds));
}

void Time::sleepSeconds(int seconds) {
    std::this_thread::sleep_for(std::chrono::seconds(seconds));
}

void Time::sleepMicroseconds(int microseconds) {
    std::this_thread::sleep_for(std::chrono::microseconds(microseconds));
}

// Timer 类实现
Timer::Timer() : running_(false) {}

void Timer::start() {
    startTime_ = std::chrono::high_resolution_clock::now();
    running_ = true;
}

void Timer::stop() {
    if (running_) {
        endTime_ = std::chrono::high_resolution_clock::now();
        running_ = false;
    }
}

void Timer::reset() {
    running_ = false;
    startTime_ = std::chrono::high_resolution_clock::time_point();
    endTime_ = std::chrono::high_resolution_clock::time_point();
}

int64_t Timer::getElapsedMilliseconds() const {
    auto end = running_ ? std::chrono::high_resolution_clock::now() : endTime_;
    auto duration = std::chrono::duration_cast<std::chrono::milliseconds>(end - startTime_);
    return duration.count();
}

int64_t Timer::getElapsedMicroseconds() const {
    auto end = running_ ? std::chrono::high_resolution_clock::now() : endTime_;
    auto duration = std::chrono::duration_cast<std::chrono::microseconds>(end - startTime_);
    return duration.count();
}

double Timer::getElapsedSeconds() const {
    return getElapsedMicroseconds() / 1000000.0;
}

bool Timer::isRunning() const {
    return running_;
}

// 全局便利函数
std::string getOperatingSystem() {
    return System::getOperatingSystem();
}

std::string getArchitecture() {
    return System::getArchitecture();
}

size_t getProcessorCount() {
    return System::getProcessorCount();
}

size_t getTotalMemory() {
    return System::getTotalMemory();
}

size_t getAvailableMemory() {
    return System::getAvailableMemory();
}

int getCurrentProcessId() {
    return System::getCurrentProcessId();
}

std::string getCurrentUser() {
    return System::getCurrentUser();
}

std::string getEnvironmentVariable(const std::string& name) {
    return System::getEnvironmentVariable(name);
}

bool setEnvironmentVariable(const std::string& name, const std::string& value) {
    return System::setEnvironmentVariable(name, value);
}

int executeCommand(const std::string& command) {
    return System::executeCommand(command);
}

std::string executeCommandWithOutput(const std::string& command) {
    return System::executeCommandWithOutput(command);
}

void sleep(int milliseconds) {
    System::sleep(milliseconds);
}

void exit(int exitCode) {
    System::exit(exitCode);
}

int64_t getCurrentTimestamp() {
    return Time::getCurrentTimestamp();
}

std::string getCurrentTimeString() {
    return Time::getCurrentTimeString();
}

} // namespace stdlib
} // namespace starry